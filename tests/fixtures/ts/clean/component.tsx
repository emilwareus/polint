import React from "react";

export { formatLabel } from "./format";

type ButtonProps = { label: string; tone?: "primary" | "secondary" };

function formatLabel(label: string) {
  return label.trim();
}

export function Button(props: ButtonProps) {
  const tone = props.tone ?? "primary";

  return (
    <button
      className="text-primary"
      aria-label={formatLabel(props.label)}
      data-tone={tone}
    >
      {props.label}
    </button>
  );
}

export class ButtonPresenter {
  render() {
    return Button({ label: "Pay", tone: "primary" });
  }
}
