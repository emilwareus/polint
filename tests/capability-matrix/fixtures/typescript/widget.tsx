export class Widget {
  name = "ok";

  label(): string {
    return this.name;
  }
}

export function Panel(props: { title: string; tone?: string }) {
  return <div className="panel" data-tone={props.tone ?? "default"}>{props.title}</div>;
}
