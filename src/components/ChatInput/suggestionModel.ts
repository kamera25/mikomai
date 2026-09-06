export interface HostSuggestion { hostname: string; ip: string; }

export function findHostSuggestions(
  query: string,
  availableHosts: HostSuggestion[],
  recentIPs: string[],
  labels: { localhost: string; pastIps: string },
): HostSuggestion[] {
  const queryLower = query.toLowerCase();
  const suggestions: HostSuggestion[] = [];
  const seenIPs = new Set<string>();
  if ("localhost".includes(queryLower) || labels.localhost.includes(query)) {
    suggestions.push({ hostname: "localhost", ip: labels.localhost });
    seenIPs.add("127.0.0.1");
    seenIPs.add("localhost");
  }
  availableHosts.forEach((host) => {
    if (host.hostname !== "localhost" && (host.hostname.toLowerCase().includes(queryLower) || host.ip.includes(query))) {
      suggestions.push(host);
    }
    seenIPs.add(host.ip);
  });
  recentIPs.forEach((ip) => {
    if ((ip.toLowerCase().includes(queryLower) || labels.pastIps.includes(query)) && !seenIPs.has(ip)) {
      suggestions.push({ hostname: ip, ip: labels.pastIps });
      seenIPs.add(ip);
    }
  });
  return suggestions;
}
