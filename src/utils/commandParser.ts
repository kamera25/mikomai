export function parsePingCommand(input: string) {
  const lowerInput = input.toLowerCase();
  const pingBaseMatch = lowerInput.match(/(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)/) ||
                        lowerInput.match(/([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)/);

  if (!pingBaseMatch) return null;

  const host = pingBaseMatch[1];
  const args: any = { host };

  const sizeMatch = lowerInput.match(/(?:size|サイズ)\s*(\d+)/);
  if (sizeMatch) args.size = parseInt(sizeMatch[1]);

  const countMatch = lowerInput.match(/(?:count|回数|回)\s*(\d+)/) || lowerInput.match(/(\d+)\s*回(?:実行)?/);
  if (countMatch) args.count = parseInt(countMatch[1]);

  if (lowerInput.includes("df") || lowerInput.includes("フラグメント禁止") || lowerInput.includes("断片化禁止")) {
    args.df = true;
  }

  return args;
}
