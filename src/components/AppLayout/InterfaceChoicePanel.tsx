import { useState } from "react";
import { InterfaceChoiceConfig } from "../../hooks/useQuestionQueue";

export interface InterfaceChoicePanelProps {
  choice: InterfaceChoiceConfig;
  progressPrefix: string;
  onSelect: (id: string, option: string) => void;
  onCancel: (id: string) => void;
}

export function InterfaceChoicePanel({
  choice,
  progressPrefix,
  onSelect,
  onCancel,
}: InterfaceChoicePanelProps) {
  const [ciscoType, setCiscoType] = useState("GigabitEthernet");
  const [ciscoNum, setCiscoNum] = useState("0/1");
  const [customInterface, setCustomInterface] = useState("");

  const vendor = choice.vendor || "Cisco_IOS";
  const isCisco =
    vendor.toLowerCase().includes("cisco") || vendor.toLowerCase().includes("ios");
  const isYamaha = vendor.toLowerCase().includes("yamaha");
  const isArista = vendor.toLowerCase().includes("arista");

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
          {progressPrefix} インターフェースの選択 - {vendor}
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

      {/* Cisco_IOS の UI */}
      {isCisco && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div style={{ display: "flex", gap: "10px" }}>
            <div
              style={{
                flex: 1,
                display: "flex",
                flexDirection: "column",
                gap: "4px",
              }}
            >
              <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>
                種別
              </label>
              <select
                value={ciscoType}
                onChange={(e) => setCiscoType(e.target.value)}
                style={{
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              >
                <option value="GigabitEthernet">GigabitEthernet</option>
                <option value="FastEthernet">FastEthernet</option>
                <option value="TenGigabitEthernet">TenGigabitEthernet</option>
                <option value="Ethernet">Ethernet</option>
                <option value="Vlan">Vlan</option>
                <option value="Loopback">Loopback</option>
              </select>
            </div>
            <div
              style={{
                flex: 1,
                display: "flex",
                flexDirection: "column",
                gap: "4px",
              }}
            >
              <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>
                番号
              </label>
              <input
                type="text"
                value={ciscoNum}
                onChange={(e) => setCiscoNum(e.target.value)}
                placeholder="例: 0/1, 1/0/1"
                style={{
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
            </div>
          </div>
          <button
            className="btn btn-primary"
            onClick={() => onSelect(choice.id, `${ciscoType}${ciscoNum}`)}
            style={{
              width: "100%",
              padding: "10px",
              fontWeight: "500",
            }}
          >
            選択を確定 : {ciscoType}
            {ciscoNum}
          </button>
        </div>
      )}

      {/* Yamaha の UI */}
      {isYamaha && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
            {["lan1", "lan2", "lan3", "lan4", "wan1", "wan2"].map((opt) => (
              <button
                key={opt}
                onClick={() => onSelect(choice.id, opt)}
                style={{
                  padding: "8px 12px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                  cursor: "pointer",
                  transition: "border-color 0.15s ease",
                }}
                onMouseEnter={(e) =>
                  (e.currentTarget.style.borderColor = "var(--primary)")
                }
                onMouseLeave={(e) =>
                  (e.currentTarget.style.borderColor = "var(--border)")
                }
              >
                {opt}
              </button>
            ))}
          </div>
          <div
            style={{
              borderTop: "1px solid var(--border)",
              paddingTop: "8px",
              display: "flex",
              flexDirection: "column",
              gap: "4px",
            }}
          >
            <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>
              カスタム入力
            </label>
            <div style={{ display: "flex", gap: "10px" }}>
              <input
                type="text"
                value={customInterface}
                onChange={(e) => setCustomInterface(e.target.value)}
                placeholder="例: lan1.1, tunnel1"
                style={{
                  flex: 1,
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
              <button
                className="btn btn-primary"
                onClick={() => {
                  if (customInterface.trim()) {
                    onSelect(choice.id, customInterface.trim());
                  }
                }}
                disabled={!customInterface.trim()}
                style={{
                  padding: "8px 16px",
                  fontWeight: "500",
                }}
              >
                確定
              </button>
            </div>
          </div>
        </div>
      )}

      {/* その他のベンダー (Cisco, Yamaha 以外) の UI */}
      {!isCisco && !isYamaha && (
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {isArista && (
            <div
              style={{
                display: "flex",
                gap: "8px",
                flexWrap: "wrap",
                marginBottom: "4px",
              }}
            >
              {["Ethernet1", "Ethernet2", "Ethernet3", "Ethernet4"].map(
                (opt) => (
                  <button
                    key={opt}
                    onClick={() => onSelect(choice.id, opt)}
                    style={{
                      padding: "6px 10px",
                      background: "var(--bg-tertiary)",
                      border: "1px solid var(--border)",
                      borderRadius: "6px",
                      color: "var(--text-primary)",
                      cursor: "pointer",
                      fontSize: "12px",
                    }}
                  >
                    {opt}
                  </button>
                )
              )}
            </div>
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
            <label style={{ fontSize: "11px", color: "var(--text-secondary)" }}>
              インターフェース名を入力してください
            </label>
            <div style={{ display: "flex", gap: "10px" }}>
              <input
                type="text"
                value={customInterface}
                onChange={(e) => setCustomInterface(e.target.value)}
                placeholder="例: Ethernet1, ge-0/0/0"
                style={{
                  flex: 1,
                  padding: "8px",
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--text-primary)",
                }}
              />
              <button
                className="btn btn-primary"
                onClick={() => {
                  if (customInterface.trim()) {
                    onSelect(choice.id, customInterface.trim());
                  }
                }}
                disabled={!customInterface.trim()}
                style={{
                  padding: "8px 16px",
                  fontWeight: "500",
                }}
              >
                確定
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
