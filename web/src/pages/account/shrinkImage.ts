/** Longest side of the picture sent to the server (it stores 192×192, so this is plenty). */
const MAX_SIDE = 512;

/**
 * Downscale a picked image in the browser so a 12-megapixel photo does not
 * travel over the wire only to be shrunk to an icon. Falls back to the
 * original file when the browser cannot decode it (the server then decides).
 */
export async function shrinkImage(file: File): Promise<Blob> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    return file;
  }
  try {
    const scale = Math.min(1, MAX_SIDE / Math.max(bitmap.width, bitmap.height));
    if (scale === 1 && file.size <= 256 * 1024) return file;
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    const ctx = canvas.getContext("2d");
    if (!ctx) return file;
    ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    return blob ?? file;
  } finally {
    bitmap.close();
  }
}
