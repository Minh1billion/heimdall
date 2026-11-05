export interface ChannelField {
  key: string;
  label: string;
  secret: boolean;
  placeholder: string;
  hint: string;
}

export const CHANNEL_CHOICES = [
  { value: "zalo", label: "Zalo" },
  { value: "telegram", label: "Telegram" },
  { value: "slack", label: "Slack" },
  { value: "email", label: "Email" },
];

export const CHANNEL_FIELDS: Record<string, ChannelField[]> = {
  zalo: [
    {
      key: "bot_token",
      label: "Bot token",
      secret: true,
      placeholder: "123456789:abc...",
      hint: "Token of the Zalo Bot created at bot.zaloplatforms.com. It is stored encrypted and never shown again.",
    },
    {
      key: "chat_id",
      label: "Chat IDs",
      secret: false,
      placeholder: "1234567890, 9876543210",
      hint: "One ID per person, separated by commas. Each person gets their own separate message. The bot must already have a chat with each person.",
    },
  ],
  telegram: [
    {
      key: "bot_token",
      label: "Bot token",
      secret: true,
      placeholder: "123456:ABC...",
      hint: "Token from BotFather. It is stored encrypted and never shown again.",
    },
    {
      key: "chat_id",
      label: "Chat ID",
      secret: false,
      placeholder: "-1001234567890",
      hint: "The chat or group that receives the report.",
    },
  ],
  slack: [
    {
      key: "webhook_url",
      label: "Webhook URL",
      secret: true,
      placeholder: "https://hooks.slack.com/services/...",
      hint: "An incoming webhook for the channel. It is stored encrypted and never shown again.",
    },
  ],
  email: [
    {
      key: "to",
      label: "Recipients",
      secret: false,
      placeholder: "a@partner.com, b@partner.com",
      hint: "One or more addresses separated by commas. The SMTP server and sender are set in Settings.",
    },
  ],
};
