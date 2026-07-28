"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { AlertCircle, CheckCircle2, ImagePlus, Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { apiUpload } from "@/lib/api";
import { cn } from "@/lib/utils";

const MAX_FILE_SIZE = 20 * 1024 * 1024;
const MAX_CONCURRENT_UPLOADS = 3;

type UploadStatus = "queued" | "uploading" | "succeeded" | "failed";

type UploadItem = {
  id: number;
  file: File;
  bucketId: string;
  status: UploadStatus;
  message: string;
};

function statusLabel(status: UploadStatus) {
  switch (status) {
    case "queued":
      return "Queued";
    case "uploading":
      return "Uploading";
    case "succeeded":
      return "Succeeded";
    case "failed":
      return "Failed";
  }
}

export function ImageUploadDropzone({
  bucketId,
  disabled = false,
  onUploaded,
}: {
  bucketId: string;
  disabled?: boolean;
  onUploaded: () => void;
}) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const nextIdRef = useRef(0);
  const [isDragging, setIsDragging] = useState(false);
  const [items, setItems] = useState<UploadItem[]>([]);

  const counts = useMemo(
    () => ({
      queued: items.filter((item) => item.status === "queued").length,
      uploading: items.filter((item) => item.status === "uploading").length,
      uploaded: items.filter((item) => item.status === "succeeded").length,
      failed: items.filter((item) => item.status === "failed").length,
    }),
    [items],
  );

  useEffect(() => {
    if (disabled) return;

    const availableSlots = MAX_CONCURRENT_UPLOADS - counts.uploading;
    if (availableSlots <= 0) return;

    const nextItems = items.filter((item) => item.status === "queued").slice(0, availableSlots);
    if (nextItems.length === 0) return;

    setItems((currentItems) =>
      currentItems.map((item) =>
        nextItems.some((nextItem) => nextItem.id === item.id) ? { ...item, status: "uploading", message: "Uploading" } : item,
      ),
    );

    for (const item of nextItems) {
      void (async () => {
        try {
          await apiUpload(`/api/buckets/${item.bucketId}/images/upload`, item.file);
          onUploaded();
          setItems((currentItems) =>
            currentItems.map((currentItem) =>
              currentItem.id === item.id
                ? { ...currentItem, status: "succeeded", message: "Succeeded" }
                : currentItem,
            ),
          );
        } catch (error) {
          setItems((currentItems) =>
            currentItems.map((currentItem) =>
              currentItem.id === item.id
                ? {
                    ...currentItem,
                    status: "failed",
                    message: error instanceof Error ? error.message : "Upload failed",
                  }
                : currentItem,
            ),
          );
        }
      })();
    }
  }, [counts.uploading, disabled, items, onUploaded]);

  function queueFiles(fileList: Iterable<File>) {
    const nextItems: UploadItem[] = [];

    for (const file of fileList) {
      if (!file.type.startsWith("image/")) {
        nextItems.push({
          id: nextIdRef.current++,
          file,
          bucketId,
          status: "failed",
          message: "Images only",
        });
        continue;
      }

      if (file.size > MAX_FILE_SIZE) {
        nextItems.push({
          id: nextIdRef.current++,
          file,
          bucketId,
          status: "failed",
          message: "Too large (max 20 MiB)",
        });
        continue;
      }

      nextItems.push({
        id: nextIdRef.current++,
        file,
        bucketId,
        status: "queued",
        message: "Queued",
      });
    }

    if (nextItems.length > 0) {
      setItems((currentItems) => [...currentItems, ...nextItems]);
    }
  }

  function handleFileSelection(event: React.ChangeEvent<HTMLInputElement>) {
    if (disabled) return;
    if (event.target.files) {
      queueFiles(Array.from(event.target.files));
    }
    event.target.value = "";
  }

  function handleDrop(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setIsDragging(false);
    if (disabled) return;

    queueFiles(Array.from(event.dataTransfer.files));
  }

  return (
    <div className="space-y-3">
      <div
        role="region"
        aria-label="Image upload"
        onDragEnter={(event) => {
          event.preventDefault();
          if (!disabled) {
            setIsDragging(true);
          }
        }}
        onDragOver={(event) => {
          event.preventDefault();
          if (!disabled) {
            event.dataTransfer.dropEffect = "copy";
          }
        }}
        onDragLeave={(event) => {
          event.preventDefault();
          if (event.currentTarget.contains(event.relatedTarget as Node | null)) {
            return;
          }
          setIsDragging(false);
        }}
        onDrop={handleDrop}
        className={cn(
          "rounded-4xl border border-dashed bg-card/70 p-4 text-sm shadow-sm transition-colors",
          disabled && "cursor-not-allowed opacity-60",
          !disabled && "border-border",
          !disabled && isDragging && "border-primary bg-primary/5",
        )}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 space-y-1">
            <div className="flex items-center gap-2 font-medium">
              <Upload className="size-4" />
              <span>Drop images here</span>
            </div>
            <p className="text-xs text-muted-foreground">PNG, JPG, GIF, WebP, and more — up to 20 MiB each.</p>
            <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
              <span>Queued: {counts.queued}</span>
              <span>Uploading: {counts.uploading}</span>
              <span>Uploaded: {counts.uploaded}</span>
              <span>Failed: {counts.failed}</span>
            </div>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={disabled}
            onClick={() => inputRef.current?.click()}
          >
            <ImagePlus className="size-4" />
            Choose images
          </Button>
        </div>
        <input
          ref={inputRef}
          type="file"
          aria-label="Select image files"
          accept="image/*"
          multiple
          className="hidden"
          disabled={disabled}
          onChange={handleFileSelection}
        />
      </div>

      {items.length > 0 ? (
        <div className="space-y-2">
          {items.map((item) => {
            const Icon = item.status === "succeeded" ? CheckCircle2 : item.status === "failed" ? AlertCircle : Loader2;
            return (
              <div
                key={item.id}
                className="flex items-center justify-between gap-3 rounded-2xl border bg-card px-3 py-2 text-xs shadow-xs"
              >
                <div className="min-w-0">
                  <p className="truncate font-medium">{item.file.name}</p>
                  {item.status === "failed" ? (
                    <p className="text-destructive">{item.message}</p>
                  ) : (
                    <p className="text-muted-foreground">{item.message}</p>
                  )}
                </div>
                <div
                  className={cn(
                    "inline-flex shrink-0 items-center gap-1.5 rounded-full px-2 py-1 font-medium",
                    item.status === "succeeded" && "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
                    item.status === "failed" && "bg-destructive/10 text-destructive",
                    (item.status === "queued" || item.status === "uploading") && "bg-muted text-muted-foreground",
                  )}
                >
                  <Icon className={cn("size-3.5", item.status === "uploading" && "animate-spin")} />
                  <span>{statusLabel(item.status)}</span>
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
