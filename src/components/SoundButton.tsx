import { ButtonHTMLAttributes } from "react";
import { AudioManager } from "../utils/audioManager";

/** A button with the standard click sound. */
export function SoundButton({
  onClick,
  type = "button",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      {...props}
      type={type}
      onClick={(event) => {
        AudioManager.playClick();
        onClick?.(event);
      }}
    />
  );
}
