import type { EmojiItem } from "./types";

export interface AuthResponse {
  success?: boolean;
  name?: string;
  token?: string;
  error?: string;
}

export interface VerifyResponse {
  valid: boolean;
  name: string;
}

export interface UploadResponse {
  id: string;
  name: string;
  size: number;
  mime: string;
}

export async function apiRegister(name: string, password?: string): Promise<AuthResponse> {
  const res = await fetch("/api/auth/register", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name, password: password || undefined }),
  });
  return await res.json();
}

export async function apiLogin(name: string, password?: string): Promise<AuthResponse> {
  const res = await fetch("/api/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name, password: password || undefined }),
  });
  return await res.json();
}

export async function apiVerify(name: string, token: string): Promise<VerifyResponse> {
  const res = await fetch("/api/auth/verify", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name, token }),
  });
  return await res.json();
}

export function apiUpload(
  file: File,
  onProgress?: (pct: number) => void
): Promise<UploadResponse> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", "/api/upload");
    if (xhr.upload && onProgress) {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) {
          onProgress((e.loaded / e.total) * 100);
        }
      };
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          resolve(JSON.parse(xhr.responseText));
        } catch (err) {
          reject(err);
        }
      } else {
        reject(new Error(xhr.responseText || `Upload failed with status ${xhr.status}`));
      }
    };
    xhr.onerror = () => reject(new Error("Network upload error"));

    const form = new FormData();
    form.append("file", file, file.name);
    xhr.send(form);
  });
}

export async function fetchEmojis(): Promise<EmojiItem[]> {
  const res = await fetch("/api/emojis");
  if (!res.ok) throw new Error("Failed to fetch emojis");
  return await res.json();
}
