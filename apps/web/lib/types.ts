export type User = {
  id: string;
  username: string | null;
  display_name: string | null;
  avatar_url: string | null;
  role: "admin" | "user";
  is_root_admin: boolean;
  permissions: {
    upload_local_images: boolean;
    view_admin_stats: boolean;
    manage_permissions: boolean;
  };
};

export type AdminIdentity = {
  provider: string;
  masked_id: string;
};

export type AdminPermissions = {
  upload_local_images: boolean;
  view_admin_stats: boolean;
  manage_permissions: boolean;
};

export type AdminUser = {
  id: string;
  username: string | null;
  display_name: string | null;
  role: "admin" | "user";
  is_root_admin: boolean;
  identities: AdminIdentity[];
  permissions: AdminPermissions;
};

export type AdminStatsSnapshot = {
  snapshot_date: string;
  user_count: number;
  bucket_count: number;
  image_link_count: number;
  unique_file_count: number | null;
  send_count: number;
  daily_send_count: number;
  b2_object_count: number | null;
  b2_bytes: number | null;
};

export type AdminStatsResponse = {
  current: AdminStatsSnapshot;
  history: AdminStatsSnapshot[];
  storage: {
    configured: boolean;
    available: boolean;
    first_complete_history_date: string | null;
  };
};

export type CategorySummary = {
  id: string;
  name: string;
  linkCount: number;
  timesUsed: number;
  lastUsedAt: string | null;
};

export type MediaLink = {
  id: string;
  url: string;
  previewStatus: "unchecked" | "ok" | "warning" | "failed";
};

export type ImageItem = {
  id: string;
  bucketId?: string;
  url: string;
  cdn_status?: string;
  title: string | null;
  favorite: boolean;
  randomWeight: number;
  tags: string[];
  sendCount: number;
  createdAt?: string;
  notes?: string | null;
};

export type GifSearchSelection = {
  url: string;
  title: string | null;
  slug: string | null;
  tags: string[];
};

export type ImageSearchResult = {
  bucketId: string;
  bucketName: string;
  image: ImageItem;
};

export type Bucket = {
  id: string;
  name: string;
  share_token: string | null;
  subscriber_count: number;
  is_subscribed: boolean;
  owner_username: string | null;
  whitelist_enabled: boolean;
  image_count: number;
  is_read_only: boolean;
};
