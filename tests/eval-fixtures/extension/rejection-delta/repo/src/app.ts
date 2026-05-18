export function renderProfile(user: { id: string; active: boolean }): string {
  const defaultFact = user.active ? "default:fact" : "default:missing";
  const extensionFact = `${defaultFact}->extension:fact`;

  return `${user.id}:${extensionFact}`;
}
