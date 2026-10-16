import { beforeEach, describe, expect, it } from "vitest";
import { dropAction, useFileDragStore } from "./fileDragStore";

describe("dropAction", () => {
  it("stages on the staged list and unstages on the unstaged list", () => {
    expect(dropAction("unstaged", "staged")).toBe("stage");
    expect(dropAction("staged", "unstaged")).toBe("unstage");
  });
  it("does nothing when dropped on its own list or outside", () => {
    expect(dropAction("staged", "staged")).toBeNull();
    expect(dropAction("unstaged", null)).toBeNull();
  });
});

describe("useFileDragStore", () => {
  beforeEach(() => useFileDragStore.getState().stop());

  it("tracks a drag from start to stop", () => {
    const s = useFileDragStore.getState();
    s.start("a.txt", "unstaged", 1, 2);
    s.setHover("staged");
    s.move(5, 6);
    expect(useFileDragStore.getState()).toMatchObject({
      drag: { path: "a.txt", from: "unstaged" },
      hoverSide: "staged",
      position: { x: 5, y: 6 },
    });
    s.stop();
    expect(useFileDragStore.getState().drag).toBeNull();
    expect(useFileDragStore.getState().hoverSide).toBeNull();
  });
});
