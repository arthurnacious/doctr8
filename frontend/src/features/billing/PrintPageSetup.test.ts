import { describe, expect, it } from 'vitest'

import { cssString, pageMarginCss } from './PrintPageSetup'

describe('print page setup', () => {
  it('repeats practice and account, and numbers pages', () => {
    const css = pageMarginCss('Practice A', 'Account 32')
    expect(css).toContain('@top-left { content: "Practice A · Account 32"')
    expect(css).toContain('counter(page) " of " counter(pages)')
  })

  it('escapes names so they can not break out of the CSS string', () => {
    expect(cssString('Dr "X" \\ Co')).toBe('"Dr \\"X\\" \\\\ Co"')
    expect(cssString('a</style><script>')).not.toContain('<')
    expect(cssString('line\nbreak')).toBe('"line break"')
  })
})
