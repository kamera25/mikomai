import { useState, useEffect, useRef } from "react";

interface UseResizablePaneOptions {
  initialSidebarWidth?: number;
  initialDiffWidth?: number;
}

export function useResizablePane(options?: UseResizablePaneOptions) {
  const [sidebarWidth, setSidebarWidth] = useState<number>(options?.initialSidebarWidth ?? 280);
  const [diffWidth, setDiffWidth] = useState<number>(options?.initialDiffWidth ?? 450);
  const [isResizingLeft, setIsResizingLeft] = useState(false);
  const [isResizingRight, setIsResizingRight] = useState(false);
  const animationFrameId = useRef<number | null>(null);

  const handleLeftMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizingLeft(true);
  };

  const handleRightMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizingRight(true);
  };

  useEffect(() => {
    if (!isResizingLeft && !isResizingRight) return;

    document.body.classList.add("is-resizing");

    const handleMouseMove = (e: MouseEvent) => {
      const clientX = e.clientX;
      const innerWidth = window.innerWidth;

      if (animationFrameId.current !== null) {
        cancelAnimationFrame(animationFrameId.current);
      }

      animationFrameId.current = requestAnimationFrame(() => {
        if (isResizingLeft) {
          // Left sidebar starts right after activity bar (60px width)
          const newWidth = Math.max(160, Math.min(600, clientX - 60));
          setSidebarWidth(newWidth);
        } else if (isResizingRight) {
          const newWidth = Math.max(
            280,
            Math.min(innerWidth * 0.7, innerWidth - clientX)
          );
          setDiffWidth(newWidth);
        }
      });
    };

    const handleMouseUp = () => {
      setIsResizingLeft(false);
      setIsResizingRight(false);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.body.classList.remove("is-resizing");
      if (animationFrameId.current !== null) {
        cancelAnimationFrame(animationFrameId.current);
      }
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isResizingLeft, isResizingRight]);

  return {
    sidebarWidth,
    diffWidth,
    isResizingLeft,
    isResizingRight,
    handleLeftMouseDown,
    handleRightMouseDown,
  };
}

