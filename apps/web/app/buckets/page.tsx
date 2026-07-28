"use client";

import { useState, Suspense } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { AppShell } from "@/components/app-shell";
import { BucketList } from "@/components/bucket-list";
import { ImageForm } from "@/components/image-form";
import { ImageList } from "@/components/image-list";
import { ImageUploadDropzone } from "@/components/image-upload-dropzone";
import { Folder, Link as LinkIcon, Settings, Trash2, Check, X, Pencil } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { SidebarProvider, Sidebar, SidebarInset, SidebarTrigger } from "@/components/ui/sidebar";
import { Bucket } from "@/lib/types";
import { ShareDialog } from "@/components/share-dialog";
import { RequireAuth } from "@/components/require-auth";
import { useUser } from "@/components/user-provider";
import { apiDelete, apiPost, apiPatch } from "@/lib/api";

function BucketsContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const { user } = useUser();
  const bucketId = searchParams.get("id");
  const [buckets, setBuckets] = useState<Bucket[]>([]);
  const [refreshKey, setRefreshKey] = useState(0);
  const [sizeIndex, setSizeIndex] = useState(2);
  const [infoOpen, setInfoOpen] = useState(false);
  const [editingName, setEditingName] = useState(false);
  const [newName, setNewName] = useState("");
  const [showDiscardNameConfirm, setShowDiscardNameConfirm] = useState(false);

  const COLUMN_CLASSES = [
    "columns-3 sm:columns-4 md:columns-5 lg:columns-6",
    "columns-2 sm:columns-3 md:columns-4 lg:columns-5",
    "columns-2 sm:columns-2 md:columns-3 lg:columns-4",
    "columns-1 sm:columns-2 md:columns-2 lg:columns-3",
    "columns-1 sm:columns-1 md:columns-2 lg:columns-2",
  ];
  const SIZE_LABELS = ["-2", "-1", "0", "+1", "+2"];
  const columnClass = COLUMN_CLASSES[sizeIndex] || COLUMN_CLASSES[2];

  const handleImageMoved = () => {
    setRefreshKey((k) => k + 1);
  };

  const activeBucket = buckets.find(p => p.id === bucketId);
  const isSubscribed = activeBucket?.is_subscribed;
  const isSystemBucket = bucketId === "all" || bucketId === "favorites" || activeBucket?.is_read_only;
  const isReadOnly = Boolean(isSubscribed);
  // The server flags every bucket named "Inbox" as read-only, for owner and
  // subscribers alike — but the owner's own Inbox is meant to accept new
  // links, same as the Picker already allows (see isWritablePickerBucket).
  const isOwnedInbox = Boolean(
    activeBucket && !isSubscribed && activeBucket.name.trim().toLowerCase() === "inbox"
  );
  const canManageActiveBucket = Boolean(activeBucket && !isSubscribed && (!isSystemBucket || isOwnedInbox));
  const canUploadLocalImages = Boolean(user?.permissions.upload_local_images);
  const showUploadDropzone = Boolean(bucketId && canManageActiveBucket && canUploadLocalImages);

  const handleDeleteBucket = async (bucket: Bucket) => {
    try {
      if (bucket.is_subscribed) {
        await apiPost(`/api/buckets/${bucket.id}/unsubscribe`, {});
      } else {
        await apiDelete(`/api/buckets/${bucket.id}`);
      }
      setInfoOpen(false);
      setRefreshKey((k) => k + 1);
      router.push("/buckets");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not process request.");
    }
  };

  const handleRenameBucket = async () => {
    if (!activeBucket || !newName.trim() || newName.trim() === activeBucket.name) {
      setEditingName(false);
      return;
    }
    try {
      await apiPatch(`/api/buckets/${activeBucket.id}`, { name: newName.trim() });
      setEditingName(false);
      setRefreshKey((k) => k + 1);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not rename bucket.");
    }
  };

  return (
    <SidebarProvider className="h-full flex flex-1 min-h-0 w-full overflow-hidden rounded-xl bg-muted/30 border relative">
      {/* Sidebar Area */}
      <Sidebar className="absolute h-full bg-transparent border-r-0 hidden md:flex" collapsible="offcanvas" variant="inset">
        <BucketList onBucketsChange={setBuckets} onImageMoved={handleImageMoved} refreshKey={refreshKey} />
      </Sidebar>
      
      {/* Inset Main Content Area */}
      <SidebarInset className="flex-1 flex flex-col m-2 rounded-xl bg-background shadow-sm border overflow-hidden">
        <header className="flex h-14 shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear">
          <div className="flex w-full items-center justify-between px-4 lg:px-6">
            <div className="flex items-center gap-2">
              <SidebarTrigger className="h-8 w-8 -ml-2 text-muted-foreground" />
              <h1 className="text-base font-medium flex items-center gap-2">
                {activeBucket ? activeBucket.name : "Images"}
                {activeBucket && !isReadOnly && !isSystemBucket && (
                  <Dialog
                    open={infoOpen}
                    onOpenChange={(open, eventDetails) => {
                      if (!open && editingName) {
                        // Escape is already handled locally by the name
                        // input (cancels just the inline edit) — only warn
                        // for the other ways to close the whole dialog.
                        eventDetails.cancel();
                        if (eventDetails.reason !== "escape-key") {
                          setShowDiscardNameConfirm(true);
                        }
                        return;
                      }
                      setInfoOpen(open);
                    }}
                  >
                    <DialogTrigger render={<Button variant="ghost" size="icon" className="h-6 w-6 ml-1"><Settings className="h-4 w-4 text-muted-foreground hover:text-foreground"/></Button>} />
                    <DialogContent className="sm:max-w-md">
                      <DialogHeader>
                        <DialogTitle>Bucket Settings</DialogTitle>
                      </DialogHeader>
                      <div className="space-y-4 text-sm mt-4">
                        <div className="flex justify-between items-center border-b pb-2">
                          <span className="text-muted-foreground">Bucket Name</span>
                          {editingName ? (
                            <div className="flex items-center gap-1">
                              <Input 
                                value={newName} 
                                onChange={(e) => setNewName(e.target.value)}
                                className="h-7 py-1 px-2 text-sm w-40"
                                autoFocus
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') handleRenameBucket();
                                  if (e.key === 'Escape') setEditingName(false);
                                }}
                              />
                              <Button variant="ghost" size="icon" className="h-6 w-6" onClick={handleRenameBucket}>
                                <Check className="h-4 w-4 text-green-500" />
                              </Button>
                              <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => setEditingName(false)}>
                                <X className="h-4 w-4 text-destructive" />
                              </Button>
                            </div>
                          ) : (
                            <div className="flex items-center gap-2 font-medium">
                              <span>{activeBucket.name}</span>
                              {!isSubscribed && (
                                <Button variant="ghost" size="icon" className="h-5 w-5 ml-1" onClick={() => { setNewName(activeBucket.name); setEditingName(true); }}>
                                  <Pencil className="h-3 w-3 text-muted-foreground" />
                                </Button>
                              )}
                            </div>
                          )}
                        </div>
                        <div className="flex justify-between border-b pb-2">
                          <span className="text-muted-foreground">Owner</span>
                          <span className="font-medium">{activeBucket.owner_username || "Unknown"}</span>
                        </div>
                        <div className="flex justify-between border-b pb-2">
                          <span className="text-muted-foreground">Subscribers</span>
                          <span className="font-medium">{activeBucket.subscriber_count}</span>
                        </div>
                        <div className="flex justify-between border-b pb-2">
                          <span className="text-muted-foreground">Role</span>
                          <span className="font-medium">{activeBucket.is_subscribed ? "Subscriber" : "Owner"}</span>
                        </div>
                        
                        <div className="pt-4 flex justify-end">
                          <Button variant="destructive" onClick={() => handleDeleteBucket(activeBucket)}>
                            <Trash2 className="w-4 h-4 mr-2" />
                            {activeBucket.is_subscribed ? "Unsubscribe" : "Delete Bucket"}
                          </Button>
                        </div>
                      </div>
                    </DialogContent>
                  </Dialog>
                )}
                <AlertDialog open={showDiscardNameConfirm} onOpenChange={setShowDiscardNameConfirm}>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Discard unsaved changes?</AlertDialogTitle>
                      <AlertDialogDescription>
                        You have an unsaved bucket name change. Closing now will discard it.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Keep editing</AlertDialogCancel>
                      <AlertDialogAction
                        onClick={() => {
                          setEditingName(false);
                          setInfoOpen(false);
                        }}
                        className="bg-destructive hover:bg-destructive/90 text-destructive-foreground"
                      >
                        Discard
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </h1>
            </div>
            {bucketId && !isReadOnly && activeBucket && !isSystemBucket && (
              <div className="flex items-center gap-2">
                <Dialog>
                  <DialogTrigger render={<Button variant="default" size="sm" className="h-8 gap-1.5" />}>
                    <LinkIcon className="h-3.5 w-3.5" />
                    <span className="hidden sm:inline">Share Settings</span>
                  </DialogTrigger>
                  <DialogContent>
                    <DialogHeader>
                      <DialogTitle>Share Bucket</DialogTitle>
                    </DialogHeader>
                    <ShareDialog 
                      bucket={activeBucket} 
                      onBucketChange={(updated) => setBuckets(buckets.map(p => p.id === updated.id ? updated : p))}
                    />
                  </DialogContent>
                </Dialog>
              </div>
            )}
          </div>
        </header>
        {/* Size toolbar */}
        {bucketId && (
          <div className="flex flex-col sm:flex-row items-start sm:items-center gap-2 sm:gap-3 px-4 lg:px-6 py-2 sm:py-0 h-auto sm:h-12 shrink-0 border-b bg-muted/30 w-full">
            <div className="flex items-center gap-3 h-8 shrink-0">
              <span className="text-xs font-medium text-muted-foreground whitespace-nowrap">
                Size
              </span>
              <div className="relative flex items-center w-24 h-6">
                <div className="absolute left-0 right-0 h-0.5 rounded-full bg-border" />
                <div
                  className="absolute left-0 h-0.5 rounded-full bg-primary transition-all duration-150"
                  style={{ width: `${(sizeIndex / (COLUMN_CLASSES.length - 1)) * 100}%` }}
                />
                {COLUMN_CLASSES.map((_, i) => (
                  <button
                    key={i}
                    type="button"
                    onClick={() => setSizeIndex(i)}
                    className="absolute flex items-center justify-center"
                    style={{ left: `${(i / (COLUMN_CLASSES.length - 1)) * 100}%`, transform: "translateX(-50%)" }}
                    aria-label={`Thumbnail size ${SIZE_LABELS[i]}`}
                  >
                    <span
                      className={`block rounded-full border-2 transition-all duration-150 ${
                        i === sizeIndex
                          ? "h-3.5 w-3.5 border-primary bg-primary shadow-sm"
                          : "h-2.5 w-2.5 border-muted-foreground/40 bg-background hover:border-primary/60"
                      }`}
                    />
                  </button>
                ))}
              </div>
            </div>
            {!isReadOnly && activeBucket && (!isSystemBucket || isOwnedInbox) && (
              <div className="w-full sm:w-auto flex-grow sm:flex-grow-0 sm:ml-auto">
                <ImageForm
                  bucketId={bucketId}
                  onCreated={() => setRefreshKey((k) => k + 1)}
                />
              </div>
            )}
          </div>
        )}
        <div className="flex-1 flex flex-col overflow-y-auto">
          {bucketId ? (
            <div className="p-6 space-y-6 max-w-7xl mx-auto w-full">
              {showUploadDropzone && (
                <ImageUploadDropzone
                  bucketId={bucketId}
                  disabled={!canUploadLocalImages}
                  onUploaded={() => setRefreshKey((k) => k + 1)}
                />
              )}
              <ImageList key={`${bucketId}:${refreshKey}`} bucketId={bucketId} columnClass={columnClass} readonly={isReadOnly} buckets={buckets} onMoveImage={handleImageMoved} onImageUpdated={handleImageMoved} />
            </div>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center p-8 text-center animate-in fade-in-50">
              <div className="h-16 w-16 rounded-full bg-muted flex items-center justify-center mb-4">
                <Folder className="h-8 w-8 text-muted-foreground/50" />
              </div>
              <h2 className="text-xl font-semibold">Select a bucket</h2>
              <p className="text-muted-foreground mt-2 max-w-sm">
                Choose a bucket from the sidebar to view and manage its images.
              </p>
            </div>
          )}
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default function BucketsPage() {
  return (
    <AppShell>
      <Suspense fallback={<div className="flex items-center justify-center h-full min-h-screen">Loading...</div>}>
        <RequireAuth>
          <BucketsContent />
        </RequireAuth>
      </Suspense>
    </AppShell>
  );
}
