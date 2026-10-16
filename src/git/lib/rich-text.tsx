import { Fragment, type ReactNode } from "react";

/** Renders a translated string whose `{name}` placeholders become elements
 * (e.g. `<code>`), for messages that mix text and markup. */
export function richText(text: string, render: (name: string) => ReactNode): ReactNode {
  return text.split(/\{(\w+)\}/).map((part, i) => <Fragment key={i}>{i % 2 ? render(part) : part}</Fragment>);
}
