import { Fragment, ReactNode } from "react";

/** Render `inline code` spans written with backticks in guide and diagnosis text. */
export function withInlineCode(text: string): ReactNode {
  return text
    .split("`")
    .map((part, index) =>
      index % 2 === 1 ? <code key={index}>{part}</code> : <Fragment key={index}>{part}</Fragment>
    );
}
