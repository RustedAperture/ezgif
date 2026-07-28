import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  bucketId: "bucket-1" as string | null,
  buckets: [] as Array<{
    id: string;
    name: string;
    share_token: string | null;
    subscriber_count: number;
    is_subscribed: boolean;
    owner_username: string | null;
    whitelist_enabled: boolean;
    image_count: number;
    is_read_only: boolean;
  }>,
  imageListInstance: 0,
  push: vi.fn(),
  useUser: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: mocks.push }),
  useSearchParams: () => ({
    get: (key: string) => (key === "id" ? mocks.bucketId : null),
  }),
}));

vi.mock("@/components/user-provider", () => ({
  useUser: mocks.useUser,
}));

vi.mock("@/components/app-shell", () => ({
  AppShell: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/require-auth", () => ({
  RequireAuth: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("@/components/image-form", () => ({
  ImageForm: () => <div data-testid="image-form">image form</div>,
}));

vi.mock("@/components/share-dialog", () => ({
  ShareDialog: () => <div>share dialog</div>,
}));

vi.mock("@/lib/api", () => ({
  apiDelete: vi.fn(),
  apiPatch: vi.fn(),
  apiPost: vi.fn(),
}));

vi.mock("@/components/bucket-list", async () => {
  const React = await import("react");

  return {
    BucketList: ({
      onBucketsChange,
    }: {
      onBucketsChange: (buckets: typeof mocks.buckets) => void;
    }) => {
      React.useEffect(() => {
        onBucketsChange(mocks.buckets);
      }, [onBucketsChange]);

      return <div data-testid="bucket-list">bucket list</div>;
    },
  };
});

vi.mock("@/components/image-list", async () => {
  const React = await import("react");

  return {
    ImageList: ({ bucketId, readonly }: { bucketId: string; readonly: boolean }) => {
      const instance = React.useMemo(() => ++mocks.imageListInstance, []);

      return (
        <div data-testid="image-list">
          image list {instance} for {bucketId} readonly={String(readonly)}
        </div>
      );
    },
  };
});

vi.mock("@/components/image-upload-dropzone", () => ({
  ImageUploadDropzone: ({
    bucketId,
    disabled,
    onUploaded,
  }: {
    bucketId: string;
    disabled?: boolean;
    onUploaded: () => void;
  }) => (
    <div data-testid="upload-dropzone">
      <span>
        upload dropzone {bucketId} disabled={String(Boolean(disabled))}
      </span>
      <button type="button" onClick={onUploaded}>
        trigger upload success
      </button>
    </div>
  ),
}));

import BucketsPage from "@/app/buckets/page";

function makeBucket(overrides: Partial<(typeof mocks.buckets)[number]> = {}) {
  return {
    id: "bucket-1",
    name: "Cats",
    share_token: null,
    subscriber_count: 0,
    is_subscribed: false,
    owner_username: "owner",
    whitelist_enabled: false,
    image_count: 4,
    is_read_only: false,
    ...overrides,
  };
}

function renderPage() {
  return render(<BucketsPage />);
}

describe("BucketsPage upload dropzone integration", () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  beforeEach(() => {
    vi.stubGlobal("matchMedia", vi.fn().mockImplementation(() => ({
      matches: false,
      media: "",
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })));
    mocks.bucketId = "bucket-1";
    mocks.buckets = [makeBucket()];
    mocks.imageListInstance = 0;
    mocks.push.mockReset();
    mocks.useUser.mockReset();
    mocks.useUser.mockReturnValue({
      loading: false,
      refreshUser: vi.fn(),
      user: {
        id: "user-1",
        username: "owner",
        display_name: "Owner",
        avatar_url: null,
        role: "user",
        is_root_admin: false,
        permissions: {
          upload_local_images: true,
        },
      },
    });
  });

  it("shows the upload dropzone for a permitted owner bucket", async () => {
    renderPage();

    await waitFor(() => expect(screen.getByTestId("upload-dropzone")).toBeTruthy());
    expect(screen.getByText("upload dropzone bucket-1 disabled=false")).toBeTruthy();
    expect(screen.getByTestId("image-form")).toBeTruthy();
    expect(screen.getByTestId("image-list")).toBeTruthy();
  });

  it("shows the upload dropzone for the owner's Inbox even when it is flagged read-only", async () => {
    mocks.buckets = [
      makeBucket({
        name: "Inbox",
        is_read_only: true,
      }),
    ];

    renderPage();

    await waitFor(() => expect(screen.getByTestId("upload-dropzone")).toBeTruthy());
    expect(screen.getByText("upload dropzone bucket-1 disabled=false")).toBeTruthy();
  });

  it("hides the upload dropzone when the user lacks upload permission", async () => {
    mocks.useUser.mockReturnValue({
      loading: false,
      refreshUser: vi.fn(),
      user: {
        id: "user-1",
        username: "owner",
        display_name: "Owner",
        avatar_url: null,
        role: "user",
        is_root_admin: false,
        permissions: {
          upload_local_images: false,
        },
      },
    });

    renderPage();

    await waitFor(() => expect(screen.getByTestId("image-list")).toBeTruthy());
    expect(screen.queryByTestId("upload-dropzone")).toBeNull();
  });

  it("hides the upload dropzone for subscribed buckets", async () => {
    mocks.buckets = [
      makeBucket({
        is_subscribed: true,
      }),
    ];

    renderPage();

    await waitFor(() => expect(screen.getByTestId("image-list")).toBeTruthy());
    expect(screen.queryByTestId("upload-dropzone")).toBeNull();
  });

  it("hides the upload dropzone for read-only system buckets", async () => {
    mocks.buckets = [
      makeBucket({
        name: "Archive",
        is_read_only: true,
      }),
    ];

    renderPage();

    await waitFor(() => expect(screen.getByTestId("image-list")).toBeTruthy());
    expect(screen.queryByTestId("upload-dropzone")).toBeNull();
  });

  it.each(["all", "favorites"])("hides the upload dropzone for the %s system view", async (systemBucketId) => {
    mocks.bucketId = systemBucketId;
    mocks.buckets = [makeBucket()];

    renderPage();

    await waitFor(() => expect(screen.getByText(`image list 1 for ${systemBucketId} readonly=false`)).toBeTruthy());
    expect(screen.queryByTestId("upload-dropzone")).toBeNull();
  });

  it("hides the upload dropzone when there is no active bucket", async () => {
    mocks.bucketId = null;
    mocks.buckets = [makeBucket()];

    renderPage();

    await waitFor(() => expect(screen.getByText("Select a bucket")).toBeTruthy());
    expect(screen.queryByTestId("upload-dropzone")).toBeNull();
  });

  it("refreshes the image list after a successful upload", async () => {
    renderPage();

    await waitFor(() => expect(screen.getByText("image list 1 for bucket-1 readonly=false")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "trigger upload success" }));

    await waitFor(() => expect(screen.getByText("image list 2 for bucket-1 readonly=false")).toBeTruthy());
  });
});
