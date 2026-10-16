const IMAGE_MIME_BY_EXT: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  svg: "image/svg+xml",
  ico: "image/x-icon",
  avif: "image/avif",
};

export function getImageMime(path: string): string | null {
  const ext = path.split(".").pop()?.toLowerCase();
  return (ext && IMAGE_MIME_BY_EXT[ext]) || null;
}
