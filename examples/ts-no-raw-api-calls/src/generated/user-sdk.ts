export class UsersSdk {
  async getUser(id: string): Promise<unknown> {
    return { id };
  }
}
