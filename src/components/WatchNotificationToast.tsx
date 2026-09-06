import { ipc } from "../platform";
import { useEffect, useState } from "react";
import "./WatchNotificationToast.css";

interface WatchNotification {
  watchId: string;
  message: string;
  emittedAt: string;
}

export function WatchNotificationToast() {
  const [notification, setNotification] = useState<WatchNotification | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisten = ipc.subscribe<WatchNotification>("watch-notification", (payload) => {
      setNotification(payload);
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setNotification(null), 10_000);
    });
    return () => {
      void unlisten.then((dispose) => dispose());
      if (timer) clearTimeout(timer);
    };
  }, []);

  if (!notification) return null;
  return (
    <aside className="watch-notification" role="status">
      <strong>Watch alert</strong>
      <span>{notification.message}</span>
      <button type="button" aria-label="Dismiss watch alert" onClick={() => setNotification(null)}>
        ×
      </button>
    </aside>
  );
}
