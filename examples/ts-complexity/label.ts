export function label(status: string, admin: boolean) {
  if (admin) {
    return "admin";
  }
  if (status === "paid") {
    return "paid";
  }
  return "draft";
}
