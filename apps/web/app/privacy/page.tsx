import { LegalList, LegalPage, LegalSection } from "@/components/legal-page";

export default function PrivacyPage() {
  return (
    <LegalPage
      title="Privacy Policy"
      description="What memeBucket collects, stores, and uses to provide Discord media buckets."
      updated="Jul 29, 2026"
    >
      <LegalSection title="Data We Collect">
        <LegalList>
          <li>Account identity, including an internal user ID and HMAC-SHA256 key derived from your Discord user ID.</li>
          <li>Discord profile display data returned by OAuth, such as display name and avatar URL.</li>
          <li>Your memeBucket username, used for sharing and whitelist features.</li>
          <li>Session records, CSRF token hashes, OAuth state cookies, and expiration timestamps.</li>
          <li>Bucket names, image or GIF URLs, optional notes, creation timestamps, and related internal IDs.</li>
          <li>Share tokens, subscriptions, subscriber counts, whitelist settings, and whitelist membership.</li>
          <li>Send history (via Discord random commands and Picker selections), including bucket name, selected URL, visibility setting, and timestamp.</li>
          <li>Admin-only aggregate stats and daily operational snapshots, including overall totals for accounts, buckets, image links, sends, and storage usage.</li>
        </LegalList>
      </LegalSection>

      <LegalSection title="How We Use Data">
        <p>
          memeBucket uses this data to authenticate you with Discord, show and manage your
          buckets, send random media through Discord commands, support sharing and
          whitelist features, maintain sessions and CSRF protection, apply rate
          limiting, support admin-only aggregate reporting about overall usage and
          storage trends, and process export or deletion requests.
        </p>
      </LegalSection>

      <LegalSection title="Sharing and Visibility">
        <p>
          Private buckets are intended to be visible only to you. If you create a share
          link or allow subscriptions, users with access may see bucket names, image
          URLs, previews, notes, your memeBucket username, and subscriber-related
          information. Discord message recipients can see media you send according
          to the message context and visibility option you choose.
        </p>
      </LegalSection>

      <LegalSection title="Third Parties">
        <p>
          memeBucket relies on Discord for OAuth, application commands, profile data, and
          message delivery. It also uses the Klipy API for the GIF search feature,
          which means your search queries are sent to Klipy. When you add a raw video
          file (like an MP4) to your bucket, memeBucket may temporarily process that file and
          automatically upload it to ImgBB (api.imgbb.com) to convert it into a hosted
          GIF. Image and GIF URLs may point to third-party hosts; loading or viewing
          them may contact those hosts. Those services have their own terms and privacy
          policies. When you submit an X/Twitter post link, memeBucket requests X's
          syndication API (cdn.syndication.twimg.com) to identify attached media, then
          re-hosts it on Backblaze B2.
        </p>
        <p className="mt-3">
          memeBucket also supports <strong>Telegram Login</strong> as an authentication
          provider. When you sign in or link your account with Telegram, the Telegram
          Login Widget sends your Telegram user ID, first name, username (if set), and
          profile photo URL to our server. This data is stored to identify your account
          and display your profile in the dashboard.
        </p>
        <p className="mt-3">
          To make media links permanent, memeBucket may automatically fetch images or
          GIFs you add from Discord (or other expiring sources) and re-upload them to{" "}
          <strong>Backblaze B2</strong> (a third-party cloud storage provider).
          Re-hosted media is then served through <strong>Cloudflare</strong> (a CDN and
          proxy provider). By adding media to memeBucket, you consent to this
          re-hosting. Backblaze B2 and Cloudflare are governed by their own terms and
          privacy policies.
        </p>
      </LegalSection>

      <LegalSection title="Retention, Export, and Deletion">
        <p>
          memeBucket keeps account, bucket, image, sharing, subscription, whitelist, and
          command history data until it is deleted, the account is deleted, or the
          maintainers remove it for operational reasons. When signed in, you can
          export owned buckets and image URLs from the dashboard. Account deletion
          removes the user record and account-linked data removed through database
          cascade rules.
        </p>
      </LegalSection>

      <LegalSection title="Security">
        <p>
          memeBucket uses a keyed hash for Discord identity instead of intentionally
          storing raw Discord user IDs. The dashboard uses secure session cookies,
          CSRF protection for state-changing requests, and rate limiting on selected
          routes. No system can guarantee perfect security.
        </p>
      </LegalSection>

      <LegalSection title="Changes and Contact">
        <p>
          This policy may be updated as memeBucket changes. Questions or deletion/export
          concerns can be sent through the project repository or maintainer contact
          listed in the README.
        </p>
      </LegalSection>
    </LegalPage>
  );
}
