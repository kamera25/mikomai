import { useState, useEffect, useRef } from "react";

interface UseResizablePaneOptions {
  initialSidebarWidth?: number;
  initialDiffWidth?: number;
  minSidebarWidth?: number;
  maxSidebarWidth?: number;
  collapseSidebarThreshold?: number;
  onSidebarCollapse?: (collapsed: boolean) => void;
  minDiffWidth?: number;
  maxDiffRatio?: number;
  collapseDiffThreshold?: number;
  onDiffCollapse?: (collapsed: boolean) => void;
}

export function useResizablePane(options?: UseResizablePaneOptions) {
  const minSidebarWidth = options?.minSidebarWidth ?? 160;
  const maxSidebarWidth = options?.maxSidebarWidth ?? 600;
  const collapseSidebarThreshold = options?.collapseSidebarThreshold ?? 80;

  const minDiffWidth = options?.minDiffWidth ?? 280;
  const maxDiffRatio = options?.maxDiffRatio ?? 0.7;
  const collapseDiffThreshold = options?.collapseDiffThreshold ?? 140;

  const [sidebarWidth, setSidebarWidth] = useState<number>(options?.initialSidebarWidth ?? 280);
  const [diffWidth, setDiffWidth] = useState<number>(options?.initialDiffWidth ?? 450);
  const [isResizingLeft, setIsResizingLeft] = useState(false);
  const [isResizingRight, setIsResizingRight] = useState(false);

  const lastSidebarWidthRef = useRef<number>(options?.initialSidebarWidth ?? 280);
  const lastDiffWidthRef = useRef<number>(options?.initialDiffWidth ?? 450);
  const isCollapsedLeftRef = useRef<boolean>(false);
  const isCollapsedRightRef = useRef<boolean>(false);

  const onSidebarCollapseRef = useRef(options?.onSidebarCollapse);
  onSidebarCollapseRef.current = options?.onSidebarCollapse;
  const onDiffCollapseRef = useRef(options?.onDiffCollapse);
  onDiffCollapseRef.current = options?.onDiffCollapse;

  const animationFrameId = useRef<number | null>(null);

  const handleLeftMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    isCollapsedLeftRef.current = false;
    setIsResizingLeft(true);
  };

  const handleRightMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    isCollapsedRightRef.current = false;
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
          const rawWidth = clientX - 60;
          if (rawWidth < collapseSidebarThreshold) {
            if (!isCollapsedLeftRef.current) {
              isCollapsedLeftRef.current = true;
              onSidebarCollapseRef.current?.(true);
            }
          } else {
            if (isCollapsedLeftRef.current) {
              isCollapsedLeftRef.current = false;
              onSidebarCollapseRef.current?.(false);
            }
            if (rawWidth < minSidebarWidth) {
              // Snag (引っかかり): stay locked at minimum width
              setSidebarWidth(minSidebarWidth);
            } else {
              const newWidth = Math.min(maxSidebarWidth, rawWidth);
              setSidebarWidth(newWidth);
              lastSidebarWidthRef.current = newWidth;
            }
          }
        } else if (isResizingRight) {
          const rawWidth = innerWidth - clientX;
          const maxDiffWidth = innerWidth * maxDiffRatio;

          if (rawWidth < collapseDiffThreshold) {
            if (!isCollapsedRightRef.current) {
              isCollapsedRightRef.current = true;
              onDiffCollapseRef.current?.(true);
            }
          } else {
            if (isCollapsedRightRef.current) {
              isCollapsedRightRef.current = false;
              onDiffCollapseRef.current?.(false);
            }
            if (rawWidth < minDiffWidth) {
              // Snag (引っかかり): stay locked at minimum width
              setDiffWidth(minDiffWidth);
            } else {
              const newWidth = Math.min(maxDiffWidth, rawWidth);
              setDiffWidth(newWidth);
              lastDiffWidthRef.current = newWidth;
            }
          }
        }
      });
    };

    const handleMouseUp = () => {
      if (isResizingLeft) {
        if (isCollapsedLeftRef.current) {
          setSidebarWidth(lastSidebarWidthRef.current || 280);
        }
        setIsResizingLeft(false);
      }
      if (isResizingRight) {
        if (isCollapsedRightRef.current) {
          setDiffWidth(lastDiffWidthRef.current || 450);
        }
        setIsResizingRight(false);
      }
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
  }, [
    isResizingLeft,
    isResizingRight,
    minSidebarWidth,
    maxSidebarWidth,
    collapseSidebarThreshold,
    minDiffWidth,
    maxDiffRatio,
    collapseDiffThreshold,
  ]);

  return {
    sidebarWidth,
    diffWidth,
    isResizingLeft,
    isResizingRight,
    handleLeftMouseDown,
    handleRightMouseDown,
  };
}

