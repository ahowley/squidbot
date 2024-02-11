CREATE OR REPLACE VIEW senders_by_player AS
SELECT sender.sender_name,
  player.player_name,
  campaign.campaign_name
FROM sender
  JOIN alias ON alias.sender_id = sender.id
  JOIN player ON player.id = alias.player_id
  JOIN campaign ON sender.campaign_id = campaign.id;