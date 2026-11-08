/**
 * Page header and numbers for the printed statement, in CSS @page margin
 * boxes so they repeat on every A4 page (supported in Chromium-based
 * browsers; others print without them). The text is the practice name and
 * a document number only, never patient details.
 */
export function pageMarginCss(practice: string, label: string): string {
  const text = `${practice} · ${label}`
  return `@media print { @page {
  @top-left { content: ${cssString(text)}; font: 8pt sans-serif; color: #404040; }
  @bottom-right { content: "Page " counter(page) " of " counter(pages); font: 8pt sans-serif; color: #404040; }
} }`
}

/** A CSS string literal: quotes, backslashes and newlines escaped. */
export function cssString(value: string): string {
  return `"${value
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/[\n\r\f]/g, ' ')
    .replace(/</g, '\\3c ')}"`
}

/** `label` names the document, e.g. "Account 32" or "Invoice 7". */
export function PrintPageSetup({
  practice,
  label,
}: {
  practice: string
  label: string
}) {
  return <style>{pageMarginCss(practice, label)}</style>
}
