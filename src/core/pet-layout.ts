export type PetWindowLayout = {
  width: number;
  height: number;
};

export function petWindowLayout(
  freeMode: boolean,
  expanded: boolean,
  scale: number,
): PetWindowLayout {
  const dimensions = freeMode
    ? expanded
      ? { width: 380, height: 330 }
      : { width: 128, height: 128 }
    : expanded
      ? { width: 420, height: 330 }
      : { width: 280, height: 104 };
  return {
    width: Math.min(420, Math.round(dimensions.width * scale)),
    height: Math.min(360, Math.round(dimensions.height * scale)),
  };
}
