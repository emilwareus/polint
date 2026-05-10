export function Panel({ mode, enabled }: { mode: string; enabled: boolean }) {
  if (!enabled) {
    return <section>Disabled</section>;
  }
  if (mode === "admin") {
    return <section>Admin</section>;
  }
  if (mode === "billing") {
    return <section>Billing</section>;
  }
  return <section>Default</section>;
}
