/** Crop a Minecraft skin PNG (data URL) to a face avatar data URL (head + hat). */

const FACE = 8; // head base / overlay size in skin texels

/**
 * Decode skin data URL → opaque face PNG data URL (scaled).
 * Returns null if the browser cannot decode the image.
 */
export function skinFaceDataUrl(
  skinDataUrl: string,
  outSize = 64,
): Promise<string | null> {
  return new Promise((resolve) => {
    if (!skinDataUrl || !skinDataUrl.startsWith("data:image")) {
      resolve(null);
      return;
    }
    const img = new Image();
    img.decoding = "async";
    img.onload = () => {
      try {
        const tw = img.naturalWidth || 64;
        const scale = tw / 64;
        const s = FACE * scale;
        const canvas = document.createElement("canvas");
        canvas.width = outSize;
        canvas.height = outSize;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          resolve(null);
          return;
        }
        ctx.imageSmoothingEnabled = false;
        // head base (8,8)
        ctx.drawImage(img, 8 * scale, 8 * scale, s, s, 0, 0, outSize, outSize);
        // hat overlay (40,8) — only if non-empty-ish; always draw for classic look
        ctx.drawImage(img, 40 * scale, 8 * scale, s, s, 0, 0, outSize, outSize);
        resolve(canvas.toDataURL("image/png"));
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = skinDataUrl;
  });
}
