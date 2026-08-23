const DEFAULT_TRANSPARENCY = 20;
const MAX_TRANSPARENCY = 80;

export function surfaceAlphaFromTransparency(value: number): number {
  const finiteValue = Number.isFinite(value) ? value : DEFAULT_TRANSPARENCY;
  const transparency = Math.max(0, Math.min(MAX_TRANSPARENCY, finiteValue));
  return (100 - transparency) / 100;
}
