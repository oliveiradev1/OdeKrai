export type PetSpriteBounds = Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>;

export type PetHitTestPayload = {
  enabled: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
};

export function petHitTestPayload(
  freeMode: boolean,
  expanded: boolean,
  bounds: PetSpriteBounds | null | undefined,
): PetHitTestPayload {
  return {
    enabled: freeMode && !expanded && Boolean(bounds),
    x: bounds?.left ?? 0,
    y: bounds?.top ?? 0,
    width: bounds?.width ?? 0,
    height: bounds?.height ?? 0,
  };
}
