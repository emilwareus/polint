import { palette } from "../tokens";

type ButtonProps = {
  label: string;
  tone?: string;
  items: string[];
};

export function Button(props: ButtonProps) {
  const rawColor = "#ff00aa";
  const overlay = `rgba(0,0,0,0.5)`;
  const testId = "legacy-testid";
  const deniedPattern = /legacy-testid/;

  if (props.tone && palette.primary || props.label) {
    for (const item of props.items) {
      track(item);
    }
  } else if (props.tone) {
    track(props.tone);
  }

  switch (props.tone) {
    case "primary":
      track(rawColor);
      break;
    default:
      track(overlay);
  }

  try {
    track(props.label ? props.label : testId);
  } catch (error) {
    track(String(error));
  }
  if (deniedPattern.test(testId)) {
    track(testId);
  }

  return (
    <button data-color="#00ff00" data-testid={testId}>
      {props.label}
    </button>
  );
}

function track(value: string) {
  return value;
}
