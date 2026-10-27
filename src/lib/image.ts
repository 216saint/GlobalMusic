// Fotos escolhidas pela pessoa (perfil, capa de playlist): recortadas no centro, quadradas, reduzidas
// e convertidas para JPEG no próprio app. Uma foto de celular de 8 MB vira ~60 KB antes de ir a
// qualquer lugar.

/** Recorta o centro da imagem em um quadrado de até `px` de lado. */
export async function squareCanvas(file: File, px: number): Promise<HTMLCanvasElement> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new Error("Não deu para ler essa imagem. Use PNG, JPEG, WebP ou GIF.");
  }
  const side = Math.min(bitmap.width, bitmap.height);
  const size = Math.min(px, side);
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  canvas
    .getContext("2d")!
    .drawImage(bitmap, (bitmap.width - side) / 2, (bitmap.height - side) / 2, side, side, 0, 0, size, size);
  bitmap.close();
  return canvas;
}

/** JPEG em bytes (para enviar ao servidor). */
export async function squareJpegBytes(file: File, px: number, quality = 0.9): Promise<Uint8Array> {
  const canvas = await squareCanvas(file, px);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", quality));
  if (!blob) throw new Error("Não deu para converter a imagem.");
  return new Uint8Array(await blob.arrayBuffer());
}

/** JPEG como data URL (para guardar junto da playlist). */
export async function squareJpegDataUrl(file: File, px: number, quality = 0.85): Promise<string> {
  return (await squareCanvas(file, px)).toDataURL("image/jpeg", quality);
}
