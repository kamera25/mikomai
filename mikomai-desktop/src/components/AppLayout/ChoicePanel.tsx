import { ChoiceConfig } from "../../hooks/useQuestionQueue";

interface ChoicePanelProps {
  choice: ChoiceConfig;
  progressPrefix: string;
  onSelect: (id: string, option: string) => void;
  onCancel: (id: string) => void;
}

export function ChoicePanel({
  choice,
  progressPrefix,
  onSelect,
  onCancel,
}: ChoicePanelProps) {
  return (
    <div
      key={choice.id}
      className="input-choice-panel"
      style={{
        background: "var(--bg-secondary)",
        border: "1px solid var(--border)",
        borderRadius: "8px",
        padding: "12px",
        boxShadow: "0 -2px 10px rgba(0,0,0,0.15)",
        display: "flex",
        flexDirection: "column",
        gap: "10px",
        animation: "fadeIn 0.2s ease",
      }}
    >
      <div
        style={{
          fontWeight: "600",
          fontSize: "13px",
          color: "var(--text-secondary)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span>
          {progressPrefix} {choice.message}
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
          スキップ (Esc)
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        {choice.options.map((opt, idx) => (
          <button
            key={idx}
            onClick={() => onSelect(choice.id, opt)}
            style={{
              display: "flex",
              alignItems: "center",
              width: "100%",
              padding: "10px 14px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              borderRadius: "6px",
              color: "var(--text-primary)",
              textAlign: "left",
              cursor: "pointer",
              fontSize: "13px",
              fontWeight: "500",
              transition: "all 0.15s ease",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = "var(--primary)";
              e.currentTarget.style.background = "var(--bg-hover)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = "var(--border)";
              e.currentTarget.style.background = "var(--bg-tertiary)";
            }}
          >
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                width: "22px",
                height: "22px",
                borderRadius: "50%",
                background: "var(--bg-secondary)",
                marginRight: "12px",
                fontSize: "11px",
                fontWeight: "bold",
                color: "var(--text-secondary)",
              }}
            >
              {idx + 1}
            </span>
            {opt}
          </button>
        ))}
      </div>
    </div>
  );
}
