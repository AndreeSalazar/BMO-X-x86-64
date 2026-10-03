// N5.9 (03-10): el de pixeles que lee SV_Position (Cyberpunk lo usa para
// leer el G-buffer en el pixel que toca y para el ruido de pantalla). En un
// destino de 8x8: x/8, y/8, z y w/4, a un R8G8B8A8.
float4 pixel(float4 p : SV_Position) : SV_Target {
  return float4(p.x / 8.0, p.y / 8.0, p.z, p.w / 4.0);
}
