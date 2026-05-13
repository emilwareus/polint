import { UsersSdk } from "./generated/user-sdk";
import { rawRequest } from "./http";

const users = new UsersSdk();

export async function loadUserViaSdk(id: string) {
  return users.getUser(id);
}

export async function loadUserWithRawHelper(id: string) {
  return rawRequest(`/users/${id}`);
}

export async function loadUserWithGlobalFetch(id: string) {
  return fetch(`/api/users/${id}`);
}

