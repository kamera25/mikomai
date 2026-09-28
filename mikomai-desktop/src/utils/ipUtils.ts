export const isGlobalIP = (ip: string): boolean => {
  const ipv4Regex = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;
  const match = ip.match(ipv4Regex);
  if (match) {
    const parts = match.slice(1).map(Number);
    if (parts.some((p) => p > 255)) return false;
    const [p0, p1] = parts;
    if (p0 === 127) return false;
    if (p0 === 10) return false;
    if (p0 === 172 && p1 >= 16 && p1 <= 31) return false;
    if (p0 === 192 && p1 === 168) return false;
    if (p0 === 169 && p1 === 254) return false;
    if (p0 === 0) return false;
    if (p0 >= 224 && p0 <= 239) return false;
    if (p0 >= 240) return false;
    return true;
  }

  const ipv6Regex =
    /^(([0-9a-fA-F]{1,4}:){7,7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:)|fe80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]{1,}|::(ffff(:0{1,4}){0,1}:){0,1}((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9]))$/;
  if (ipv6Regex.test(ip)) {
    const cleanIp = ip.toLowerCase();
    if (cleanIp === "::1" || cleanIp === "0:0:0:0:0:0:0:1") return false;
    if (
      cleanIp.startsWith("fe8") ||
      cleanIp.startsWith("fe9") ||
      cleanIp.startsWith("fea") ||
      cleanIp.startsWith("feb")
    )
      return false;
    if (cleanIp.startsWith("fc") || cleanIp.startsWith("fd")) return false;
    if (cleanIp.startsWith("ff")) return false;
    return true;
  }
  return false;
};
