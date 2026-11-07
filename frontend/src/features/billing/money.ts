/**
 * Rand amounts. Stored as integer cents; shown as "R 1 234.56" with a
 * thin space between thousands and a point for cents, which reads well in
 * aligned columns. Negative amounts (money in, credits) get a minus sign.
 */
const THIN = String.fromCharCode(0x202f) // narrow no-break space

export function formatRand(cents: number): string {
  const sign = cents < 0 ? '-' : ''
  const abs = Math.abs(Math.round(cents))
  const rands = Math.floor(abs / 100)
  const c = String(abs % 100).padStart(2, '0')
  const grouped = String(rands).replace(/\B(?=(\d{3})+(?!\d))/g, THIN)
  return `${sign}R${THIN}${grouped}.${c}`
}

/** Blank for zero, so columns of mostly-empty amounts stay readable. */
export function formatRandOrBlank(cents: number): string {
  return cents === 0 ? '' : formatRand(cents)
}
