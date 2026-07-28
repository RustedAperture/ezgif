import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "@/lib/api";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

function imageFile(name: string, size = 16) {
  return new File([new Uint8Array(size)], name, { type: "image/png" });
}

function droppedFiles(files: File[]) {
  return {
    dataTransfer: {
      files,
      items: files.map((file) => ({
        kind: "file",
        type: file.type,
        getAsFile: () => file,
      })),
      types: ["Files"],
    },
  };
}

const fetchMock = vi.fn<typeof fetch>();

describe("apiUpload", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockReset();
    document.cookie = "csrf_token=test-csrf";
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    document.cookie = "csrf_token=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/";
  });

  it("sends multipart uploads with csrf, credentials, and json accept headers", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ id: "image-1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    expect(api.apiUpload).toBeTypeOf("function");

    const file = imageFile("cat.png");
    const response = await api.apiUpload<{ id: string }>("/api/buckets/bucket-1/images/upload", file);

    expect(response).toEqual({ id: "image-1" });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    const [path, init] = fetchMock.mock.calls[0]!;
    expect(path).toBe("/api/buckets/bucket-1/images/upload");
    expect(init?.method).toBe("POST");
    expect(init?.credentials).toBe("include");
    expect(init?.headers).toEqual({
      accept: "application/json",
      "X-CSRF-Token": "test-csrf",
    });
    expect("content-type" in ((init?.headers as Record<string, string>) ?? {})).toBe(false);
    expect(init?.body).toBeInstanceOf(FormData);
    expect((init?.body as FormData).get("file")).toBe(file);
  });

  it("redirects unauthorized uploads to login", async () => {
    fetchMock.mockResolvedValueOnce(new Response("", { status: 401 }));
    vi.stubGlobal("window", { location: { href: "http://localhost:3000/" } });

    expect(api.apiUpload).toBeTypeOf("function");

    await expect(api.apiUpload("/api/buckets/bucket-1/images/upload", imageFile("cat.png"))).rejects.toThrow(
      "Unauthorized",
    );
    expect(window.location.href).toBe("/login");
  });

  it("surfaces text error responses", async () => {
    fetchMock.mockResolvedValueOnce(new Response("Too Large", { status: 413 }));

    expect(api.apiUpload).toBeTypeOf("function");

    await expect(api.apiUpload("/api/buckets/bucket-1/images/upload", imageFile("cat.png"))).rejects.toThrow(
      "Too Large",
    );
  });
});

describe("ImageUploadDropzone", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockReset();
    document.cookie = "csrf_token=test-csrf";
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    document.cookie = "csrf_token=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/";
  });

  it("uploads images selected through the hidden file input", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ id: "image-1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    const { ImageUploadDropzone } = await import("@/components/image-upload-dropzone");
    const onUploaded = vi.fn();

    render(<ImageUploadDropzone bucketId="bucket-1" onUploaded={onUploaded} />);

    const input = screen.getByLabelText("Select image files");
    fireEvent.change(input, { target: { files: [imageFile("selected.png")] } });

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByText("Uploaded: 1")).toBeTruthy());

    expect(screen.getByText("selected.png")).toBeTruthy();
    expect(onUploaded).toHaveBeenCalledTimes(1);
  });

  it("uploads a dropped image and reports success", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ id: "image-1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    const { ImageUploadDropzone } = await import("@/components/image-upload-dropzone");
    const onUploaded = vi.fn();

    render(<ImageUploadDropzone bucketId="bucket-1" onUploaded={onUploaded} />);

    fireEvent.drop(screen.getByRole("region", { name: "Image upload" }), droppedFiles([imageFile("cat.png")]));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByText("Uploaded: 1")).toBeTruthy());

    expect(screen.getByText("cat.png")).toBeTruthy();
    expect(screen.getAllByText("Succeeded")).toHaveLength(2);
    expect(onUploaded).toHaveBeenCalledTimes(1);
  });

  it("rejects oversized files before requesting upload", async () => {
    const { ImageUploadDropzone } = await import("@/components/image-upload-dropzone");

    render(<ImageUploadDropzone bucketId="bucket-1" onUploaded={vi.fn()} />);

    const oversized = imageFile("huge.png", 20 * 1024 * 1024 + 1);
    fireEvent.drop(screen.getByRole("region", { name: "Image upload" }), droppedFiles([oversized]));

    await waitFor(() => expect(screen.getByText("Failed: 1")).toBeTruthy());

    expect(fetchMock).not.toHaveBeenCalled();
    expect(screen.getByText("huge.png")).toBeTruthy();
    expect(screen.getByText("Too large (max 20 MiB)")).toBeTruthy();
  });

  it("starts at most three uploads at a time and keeps the rest queued", async () => {
    const pending: Array<ReturnType<typeof deferred<Response>>> = [];
    fetchMock.mockImplementation(() => {
      const request = deferred<Response>();
      pending.push(request);
      return request.promise;
    });

    const { ImageUploadDropzone } = await import("@/components/image-upload-dropzone");
    const onUploaded = vi.fn();

    render(<ImageUploadDropzone bucketId="bucket-1" onUploaded={onUploaded} />);

    fireEvent.drop(
      screen.getByRole("region", { name: "Image upload" }),
      droppedFiles([
        imageFile("one.png"),
        imageFile("two.png"),
        imageFile("three.png"),
        imageFile("four.png"),
        imageFile("five.png"),
      ]),
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(3));
    expect(screen.getByText("Uploading: 3")).toBeTruthy();
    expect(screen.getByText("Queued: 2")).toBeTruthy();

    pending[0]!.resolve(
      new Response(JSON.stringify({ id: "image-1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(4));
    await waitFor(() => expect(screen.getByText("Queued: 1")).toBeTruthy());

    for (const request of pending.slice(1)) {
      request.resolve(
        new Response(JSON.stringify({ id: "image-ok" }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      );
    }

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(5));
    pending[4]!.resolve(
      new Response(JSON.stringify({ id: "image-ok" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    await waitFor(() => expect(screen.getByText("Uploaded: 5")).toBeTruthy());
    expect(onUploaded).toHaveBeenCalledTimes(5);
  });

  it("continues uploading later files after one upload fails", async () => {
    const pending: Array<ReturnType<typeof deferred<Response>>> = [];
    fetchMock.mockImplementation(() => {
      const request = deferred<Response>();
      pending.push(request);
      return request.promise;
    });

    const { ImageUploadDropzone } = await import("@/components/image-upload-dropzone");
    const onUploaded = vi.fn();

    render(<ImageUploadDropzone bucketId="bucket-1" onUploaded={onUploaded} />);

    fireEvent.drop(
      screen.getByRole("region", { name: "Image upload" }),
      droppedFiles([imageFile("good-1.png"), imageFile("bad.png"), imageFile("good-2.png"), imageFile("good-3.png")]),
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(3));

    pending[0]!.resolve(
      new Response(JSON.stringify({ id: "image-1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    pending[1]!.resolve(new Response("Upload failed", { status: 500 }));
    pending[2]!.resolve(
      new Response(JSON.stringify({ id: "image-2" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(4));

    pending[3]!.resolve(
      new Response(JSON.stringify({ id: "image-3" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    await waitFor(() => expect(screen.getByText("Uploaded: 3")).toBeTruthy());
    await waitFor(() => expect(screen.getByText("Failed: 1")).toBeTruthy());

    expect(screen.getByText("bad.png")).toBeTruthy();
    expect(screen.getByText("Upload failed")).toBeTruthy();
    expect(onUploaded).toHaveBeenCalledTimes(3);
  });
});
