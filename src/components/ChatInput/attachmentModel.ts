export function isImageFile(file: File): boolean {
  return Boolean(file.type?.startsWith("image/")) || isImagePath(file.name);
}

export function isImagePath(filePath: string): boolean {
  return /\.(png|jpg|jpeg|gif|webp|bmp|svg|heic|heif|tiff)$/i.test(filePath);
}
