function getCsrfToken(): string {
  if (typeof document === "undefined") return "";
  const match = document.cookie.match(/(?:^|;\s*)csrf_token=([^;]*)/);
  return match ? match[1] : "";
}

export async function apiGet<T>(path: string): Promise<T> {
  const response = await fetch(path, {
    credentials: "include",
    headers: { accept: "application/json" },
  });

  if (response.status === 401) {
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<T>;
}

export async function apiPost<TRequest, TResponse>(path: string, body: TRequest): Promise<TResponse> {
  const response = await fetch(path, {
    method: "POST",
    credentials: "include",
    headers: { 
      "content-type": "application/json", 
      accept: "application/json",
      "X-CSRF-Token": getCsrfToken(),
    },
    body: JSON.stringify(body),
  });

  if (response.status === 401) {
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<TResponse>;
}

export async function apiUpload<TResponse>(path: string, file: File): Promise<TResponse> {
  const formData = new FormData();
  formData.append("file", file);

  const response = await fetch(path, {
    method: "POST",
    credentials: "include",
    headers: {
      accept: "application/json",
      "X-CSRF-Token": getCsrfToken(),
    },
    body: formData,
  });

  if (response.status === 401) {
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<TResponse>;
}

export async function apiPatch<TRequest, TResponse>(path: string, body: TRequest): Promise<TResponse> {
  const response = await fetch(path, {
    method: "PATCH",
    credentials: "include",
    headers: { 
      "content-type": "application/json", 
      accept: "application/json",
      "X-CSRF-Token": getCsrfToken(),
    },
    body: JSON.stringify(body),
  });

  if (response.status === 401) {
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    throw new Error(await response.text());
  }

  if (response.status === 204 || response.headers.get("content-length") === "0") {
    return undefined as TResponse;
  }

  return response.json() as Promise<TResponse>;
}

export async function apiDelete<TResponse>(path: string, body?: any): Promise<TResponse> {
  const response = await fetch(path, {
    method: "DELETE",
    credentials: "include",
    headers: { 
      accept: "application/json",
      ...(body ? { "content-type": "application/json" } : {}),
      "X-CSRF-Token": getCsrfToken(),
    },
    body: body ? JSON.stringify(body) : undefined,
  });

  if (response.status === 401) {
    window.location.href = "/login";
    throw new Error("Unauthorized");
  }

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<TResponse>;
}
