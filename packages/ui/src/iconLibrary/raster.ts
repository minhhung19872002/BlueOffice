/**
 * PNG rendition of a library icon for the documents that store raster pictures only
 * (BlueDoc inline pictures, BlueSheet pictures in cells). Drawn by the web view's own SVG
 * renderer on a canvas — the same code path on the web and in the desktop shell.
 */

/** Renders an SVG document into a square PNG of `size` pixels (transparent background). */
export async function svgToPng(svg: string, size: number): Promise<Uint8Array> {
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error('icon: the SVG could not be decoded'));
      img.src = url;
    });
    const canvas = document.createElement('canvas');
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('icon: no 2D canvas');
    ctx.drawImage(img, 0, 0, size, size);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
    if (!blob) throw new Error('icon: PNG encoding failed');
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}
