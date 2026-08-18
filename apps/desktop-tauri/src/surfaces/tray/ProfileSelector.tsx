import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent as ReactMouseEvent } from "react";
import type { ProfileSummaryDto } from "../../types/bridge";
import type { TrayCopy } from "./copy";

function cleanIdentity(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed || /^current[\s_-]*cli$/i.test(trimmed)) return null;
  return trimmed;
}

function profileOptionLabel(
  profile: ProfileSummaryDto,
  copy: TrayCopy,
): string {
  if (profile.kind === "currentCli") {
    return (
      cleanIdentity(profile.accountDisplayName) ??
      cleanIdentity(profile.accountEmail) ??
      cleanIdentity(profile.email) ??
      cleanIdentity(profile.label) ??
      copy.signedOut
    );
  }
  return profile.label;
}

interface ProfileSelectorProps {
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  copy: TrayCopy;
  onSelect(profileId: string): Promise<void> | void;
  autoFocus?: boolean;
  disabled?: boolean;
}

export default function ProfileSelector({
  profiles,
  selectedProfileId,
  copy,
  onSelect,
  autoFocus = false,
  disabled = false,
}: ProfileSelectorProps) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(() =>
    Math.max(
      0,
      profiles.findIndex((profile) => profile.id === selectedProfileId),
    ),
  );
  const triggerRef = useRef<HTMLButtonElement>(null);
  const optionRefs = useRef<Array<HTMLLIElement | null>>([]);
  const listboxId = "profile-selector-listbox";
  const triggerDisabled = disabled || profiles.length === 0;
  const selectedIndex = Math.max(
    0,
    profiles.findIndex((profile) => profile.id === selectedProfileId),
  );

  useEffect(() => {
    if (autoFocus && !triggerDisabled) triggerRef.current?.focus();
  }, [autoFocus, triggerDisabled]);

  useEffect(() => {
    if (!open) return;
    const nextIndex = Math.min(activeIndex, Math.max(0, profiles.length - 1));
    setActiveIndex(nextIndex);
    optionRefs.current[nextIndex]?.focus();
  }, [activeIndex, open, profiles.length]);

  useEffect(() => {
    if (!open) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      const section = triggerRef.current?.closest(".tray-account");
      if (section && target instanceof Node && !section.contains(target)) {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [open]);

  const close = () => {
    setOpen(false);
    triggerRef.current?.focus();
  };

  const choose = async (profileId: string) => {
    close();
    await onSelect(profileId);
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (triggerDisabled) return;
    if (event.key === "Enter" || event.key === " " || event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex(selectedIndex);
      setOpen(true);
    } else if (event.key === "Escape" && open) {
      event.preventDefault();
      close();
    }
  };

  const handleOptionKeyDown = (
    event: KeyboardEvent<HTMLLIElement>,
    index: number,
    profileId: string,
  ) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex(Math.min(profiles.length - 1, index + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex(Math.max(0, index - 1));
    } else if (event.key === "Home") {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === "End") {
      event.preventDefault();
      setActiveIndex(Math.max(0, profiles.length - 1));
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void choose(profileId);
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  };

  const selected = profiles[selectedIndex];
  const selectedLabel = selected
    ? `${profileOptionLabel(selected, copy)}${selected.kind === "managed" ? ` · ${copy.managed}` : ""}`
    : copy.noProfiles;

  return (
    <section
      className="tray-region tray-account tray-account--card"
      role="region"
      aria-label={copy.account}
    >
      <label id="profile-selector-label">{copy.profile}</label>
      <div className="tray-account__control">
        <button
          ref={triggerRef}
          type="button"
          className="tray-account__trigger"
          aria-label={copy.profile}
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-controls={listboxId}
          disabled={triggerDisabled}
          autoFocus={autoFocus && !triggerDisabled}
          onClick={() => {
            setActiveIndex(selectedIndex);
            setOpen((value) => !value);
          }}
          onKeyDown={handleTriggerKeyDown}
        >
          <span className="tray-account__selected">{selectedLabel}</span>
          <span className="tray-account__chevron" aria-hidden="true">
            {open ? "⌃" : "⌄"}
          </span>
        </button>
        {open && profiles.length > 0 ? (
          <ul
            ref={(node) => {
              if (node) {
                const first = optionRefs.current[activeIndex];
                if (first) first.focus();
              }
            }}
            id={listboxId}
            className="tray-account__listbox"
            role="listbox"
            aria-labelledby="profile-selector-label"
          >
            {profiles.map((profile, index) => {
              const label = `${profileOptionLabel(profile, copy)}${profile.kind === "managed" ? ` · ${copy.managed}` : ""}`;
              return (
                <li
                  key={profile.id}
                  ref={(node) => {
                    optionRefs.current[index] = node;
                  }}
                  className="tray-account__option"
                  role="option"
                  aria-selected={profile.id === selectedProfileId}
                  tabIndex={index === activeIndex ? 0 : -1}
                  onClick={(event: ReactMouseEvent<HTMLLIElement>) => {
                    event.preventDefault();
                    void choose(profile.id);
                  }}
                  onKeyDown={(event) => handleOptionKeyDown(event, index, profile.id)}
                >
                  <span>{label}</span>
                  {profile.id === selectedProfileId ? (
                    <span aria-hidden="true">✓</span>
                  ) : null}
                </li>
              );
            })}
          </ul>
        ) : null}
      </div>
    </section>
  );
}
