export type AsyncState<T> = { status: "idle" | "loading" | "ready" | "error"; data?: T; error?: string };
