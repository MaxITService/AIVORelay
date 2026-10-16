/**
 * Calls onOutside when a mouse press starts outside the element. A press, not a
 * click: the field clicked next starts its own capture on click, so this one
 * must stop first. Returns the unsubscribe function.
 */
export function listenForPressOutside(
  target: EventTarget,
  isInside: (node: Node) => boolean,
  onOutside: () => void,
): () => void {
  const handlePress = (event: Event) => {
    if (!isInside(event.target as Node)) onOutside();
  };

  target.addEventListener("mousedown", handlePress);
  return () => target.removeEventListener("mousedown", handlePress);
}
