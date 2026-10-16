import md5 from "blueimp-md5";

export function gravatarUrl(email: string, size = 32): string {
  const hash = md5(email.trim().toLowerCase());
  return `https://www.gravatar.com/avatar/${hash}?d=identicon&s=${size}`;
}
