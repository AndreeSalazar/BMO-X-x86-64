// N5.7 (03-10): `clip` y `discard`, como los usa Cyberpunk (el follaje, el
// pelo, las rejas: lo recortado por alfa). Sale el color solo si el pixel
// queda.
float4 pixel(float4 p : SV_Position, float4 x : TEXCOORD0) : SV_Target {
  clip(x.x - 0.5);
  if (x.y > 0.75)
    discard;
  return float4(x.x, x.y, 0.0, 1.0);
}
