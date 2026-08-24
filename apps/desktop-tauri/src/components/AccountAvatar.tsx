import { useState } from "react";
import type { CSSProperties } from "react";
import ChatGptMark from "../theme/ChatGptMark";
import type { ProfileSummaryDto } from "../types/bridge";

export type AccountAvatarIdentity = Pick<
  ProfileSummaryDto,
  "presentationName" | "avatarKind" | "avatarAssetUri"
>;

export interface AccountAvatarProps {
  identity: AccountAvatarIdentity | null;
  size: number;
  decorative: boolean;
}

export default function AccountAvatar({
  identity,
  size,
  decorative,
}: AccountAvatarProps) {
  const [failedAssetUri, setFailedAssetUri] = useState<string | null>(null);
  const assetUri = identity?.avatarAssetUri ?? null;
  const avatarKind = identity?.avatarKind ?? "default";
  const showImage =
    avatarKind !== "default" && assetUri !== null && assetUri !== failedAssetUri;
  const renderedKind = showImage ? avatarKind : "default";
  const accessibility = decorative
    ? { "aria-hidden": true as const }
    : { "aria-label": identity?.presentationName ?? "codex-barbar" };

  return (
    <span
      {...accessibility}
      className="account-avatar"
      data-avatar-kind={renderedKind}
      style={{ "--account-avatar-size": `${size}px` } as CSSProperties}
    >
      {showImage ? (
        <img
          className="account-avatar__image"
          src={assetUri}
          alt={decorative ? "" : identity?.presentationName ?? "codex-barbar"}
          draggable={false}
          onError={() => setFailedAssetUri(assetUri)}
        />
      ) : (
        <ChatGptMark className="account-avatar__mark" />
      )}
    </span>
  );
}
