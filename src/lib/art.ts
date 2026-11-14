// Local images use the backend's bounded, allowlisted asset protocol. Encode each path
// segment, including CSS url() punctuation, so filenames cannot alter a background style.
export function artUrl(path: string): string {
  if (!path.startsWith("/")) return "";
  return "omnideck://localhost" + path.split("/").map((segment) =>
    encodeURIComponent(segment).replace(/[!'()*]/g, (c) =>
      `%${c.charCodeAt(0).toString(16).toUpperCase()}`),
  ).join("/");
}
