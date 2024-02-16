CREATE OR REPLACE PROCEDURE clear_all_tables() LANGUAGE plpgsql AS $$ BEGIN TRUNCATE roll_single,
  roll,
  chat_message,
  post,
  alias,
  sender,
  campaign,
  censor,
  pronouns_map,
  player,
  pronouns;

END;

$$;