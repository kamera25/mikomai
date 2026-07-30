import { useState, useEffect } from "react";
import { IpAddressChoiceConfig } from "../../hooks/useQuestionQueue";

export interface IpAddressChoicePanelProps {
  choice: IpAddressChoiceConfig;
  progressPrefix: string;
  onSelect: (id: string, option: string) => void;
  onCancel: (id: string) => void;
}

export function ip2long(ip: string): number {
  const parts = ip.split(".").map(Number);
  if (
    parts.length !== 4 ||
    parts.some(isNaN) ||
    parts.some((p) => p < 0 || p > 255)
  ) {
    return -1;
  }
  return (
    ((parts[0] << 24) >>> 0) +
    (parts[1] << 16) +
    (parts[2] << 8) +
    parts[3]
  );
}

export function isIpInSubnet(ip: string, subnet: string): boolean {
  const ipLong = ip2long(ip);
  if (ipLong === -1) return false;

  const parts = subnet.split("/");
  const subnetIp = parts[0];
  const subnetIpLong = ip2long(subnetIp);
  if (subnetIpLong === -1) return false;

  let maskLength = 32;
  if (parts.length > 1) {
    const maskStr = parts[1];
    if (maskStr.includes(".")) {
      const maskLong = ip2long(maskStr);
      if (maskLong === -1) return false;
      return (ipLong & maskLong) === (subnetIpLong & maskLong);
    } else {
      maskLength = parseInt(maskStr, 10);
      if (isNaN(maskLength) || maskLength < 0 || maskLength > 32) return false;
    }
  }

  if (maskLength === 0) return true;
  const mask = maskLength === 32 ? 0xffffffff : ~((1 << (32 - maskLength)) - 1);
  return (ipLong & mask) === (subnetIpLong & mask);
}

export function validateIpAndSubnet(
  ip: string,
  subnetInput: string,
  requiredSubnet?: string
): { isValid: boolean; error?: string } {
  const ipLong = ip2long(ip);
  if (ipLong === -1) {
    return {
      isValid: false,
      error: "無効なIPアドレスの形式です (例: 192.168.1.1)",
    };
  }

  let isValidMask = false;
  let maskText = subnetInput.trim();
  if (maskText.startsWith("/")) {
    maskText = maskText.substring(1);
  }

  if (/^\d+$/.test(maskText)) {
    const num = parseInt(maskText, 10);
    if (num >= 0 && num <= 32) {
      isValidMask = true;
    }
  } else {
    const maskLong = ip2long(maskText);
    if (maskLong !== -1) {
      const inv = ~maskLong >>> 0;
      if (((inv + 1) & inv) === 0) {
        isValidMask = true;
      }
    }
  }

  if (!isValidMask && maskText !== "") {
    return {
      isValid: false,
      error:
        "無効なサブネットマスクまたはプレフィックス長です (例: 255.255.255.0 または 24)",
    };
  }

  if (requiredSubnet && requiredSubnet.trim() !== "") {
    if (requiredSubnet.includes("/")) {
      const parts = requiredSubnet.split("/");
      const netIp = parts[0];
      if (ip2long(netIp) !== -1) {
        if (!isIpInSubnet(ip, requiredSubnet)) {
          return {
            isValid: false,
            error: `IPアドレスは指定されたサブネット範囲 (${requiredSubnet}) 内である必要があります`,
          };
        }
      }
    }
  }

  return { isValid: true };
}

export function IpAddressChoicePanel({
  choice,
  progressPrefix,
  onSelect,
  onCancel,
}: IpAddressChoicePanelProps) {
  const initialIp = choice.defaultIp || "";
  let initialSubnet = "";
  if (choice.subnet) {
    if (choice.subnet.includes("/")) {
      initialSubnet = choice.subnet.split("/")[1];
    } else {
      initialSubnet = choice.subnet;
    }
  } else {
    initialSubnet = "24";
  }

  const [ipAddress, setIpAddress] = useState(initialIp);
  const [subnetMask, setSubnetMask] = useState(initialSubnet);
  const [validationError, setValidationError] = useState<string | undefined>(
    undefined
  );

  useEffect(() => {
    if (!ipAddress && !subnetMask) {
      setValidationError(undefined);
      return;
    }
    const result = validateIpAndSubnet(ipAddress, subnetMask, choice.subnet);
    if (!result.isValid) {
      setValidationError(result.error);
    } else {
      setValidationError(undefined);
    }
  }, [ipAddress, subnetMask, choice.subnet]);

  const handleSubmit = () => {
    const result = validateIpAndSubnet(ipAddress, subnetMask, choice.subnet);
    if (result.isValid) {
      let formattedSubnet = subnetMask.trim();
      if (formattedSubnet.startsWith("/")) {
        formattedSubnet = formattedSubnet.substring(1);
      }
      const isPrefix = /^\d+$/.test(formattedSubnet);

      let output = "";
      if (isPrefix) {
        output = `${ipAddress}/${formattedSubnet}`;
      } else {
        output = `${ipAddress} ${formattedSubnet}`;
      }
      onSelect(choice.id, output);
    }
  };

  const isSubnetCidr = choice.subnet && choice.subnet.includes("/");

  return (
    <div
      className="input-choice-panel"
      style={{
        background: "var(--bg-secondary)",
        border: "1px solid var(--border)",
        borderRadius: "8px",
        padding: "16px",
        boxShadow: "0 -2px 10px rgba(0,0,0,0.15)",
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        animation: "fadeIn 0.2s ease",
      }}
    >
      <div
        style={{
          fontWeight: "600",
          fontSize: "14px",
          color: "var(--text-primary)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span>
          {progressPrefix} {choice.title || "IPアドレスの設定"}
        </span>
        <button
          onClick={() => onCancel(choice.id)}
          style={{
            background: "transparent",
            border: "none",
            color: "var(--text-secondary)",
            cursor: "pointer",
            fontSize: "11px",
            padding: "2px 6px",
            borderRadius: "4px",
          }}
          onMouseEnter={(e) =>
            (e.currentTarget.style.background = "var(--bg-tertiary)")
          }
          onMouseLeave={(e) =>
            (e.currentTarget.style.background = "transparent")
          }
        >
          キャンセル (Esc)
        </button>
      </div>

      {choice.message && (
        <div
          style={{
            fontSize: "13px",
            color: "var(--text-secondary)",
            marginBottom: "4px",
            whiteSpace: "pre-wrap",
          }}
        >
          {choice.message}
        </div>
      )}

      {isSubnetCidr && (
        <div
          style={{
            fontSize: "12px",
            background: "rgba(59, 130, 246, 0.1)",
            border: "1px solid rgba(59, 130, 246, 0.2)",
            color: "var(--primary)",
            padding: "8px 12px",
            borderRadius: "6px",
            fontWeight: "500",
            display: "flex",
            alignItems: "center",
            gap: "6px",
          }}
        >
          <span style={{ fontSize: "14px" }}>ℹ️</span>
          <span>
            要求サブネット範囲: <strong>{choice.subnet}</strong>
          </span>
        </div>
      )}

      <div style={{ display: "flex", gap: "10px" }}>
        <div
          style={{
            flex: 2,
            display: "flex",
            flexDirection: "column",
            gap: "4px",
          }}
        >
          <label
            style={{
              fontSize: "11px",
              color: "var(--text-secondary)",
              fontWeight: "500",
            }}
          >
            IPアドレス
          </label>
          <input
            type="text"
            value={ipAddress}
            onChange={(e) => setIpAddress(e.target.value)}
            placeholder="例: 192.168.1.1"
            style={{
              padding: "10px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              borderRadius: "6px",
              color: "var(--text-primary)",
              fontSize: "13px",
            }}
          />
        </div>
        <div
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            gap: "4px",
          }}
        >
          <label
            style={{
              fontSize: "11px",
              color: "var(--text-secondary)",
              fontWeight: "500",
            }}
          >
            サブネットマスク / プレフィックス
          </label>
          <input
            type="text"
            value={subnetMask}
            onChange={(e) => setSubnetMask(e.target.value)}
            placeholder="例: 24, 255.255.255.0"
            style={{
              padding: "10px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              borderRadius: "6px",
              color: "var(--text-primary)",
              fontSize: "13px",
            }}
          />
        </div>
      </div>

      {validationError && (
        <div
          style={{
            fontSize: "12px",
            color: "#ef4444",
            background: "rgba(239, 68, 68, 0.08)",
            padding: "8px 12px",
            borderRadius: "6px",
            border: "1px solid rgba(239, 68, 68, 0.15)",
            display: "flex",
            alignItems: "center",
            gap: "6px",
          }}
        >
          <span>⚠️</span>
          <span>{validationError}</span>
        </div>
      )}

      <button
        className="btn btn-primary"
        onClick={handleSubmit}
        disabled={!!validationError || !ipAddress || !subnetMask}
        style={{
          width: "100%",
          padding: "10px",
          fontWeight: "500",
          marginTop: "4px",
          opacity: validationError || !ipAddress || !subnetMask ? 0.6 : 1,
          cursor:
            validationError || !ipAddress || !subnetMask
              ? "not-allowed"
              : "pointer",
        }}
      >
        設定を確定
      </button>
    </div>
  );
}
