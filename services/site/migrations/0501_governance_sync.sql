-- People whose director or lead role in a committee was removed or changed on the People page.
-- The governance sync gives them no role in that committee again while governance still lists
-- them there, and deletes the row once it stops.
CREATE TABLE governance_opt_outs (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    committee text NOT NULL REFERENCES committees (slug),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, committee)
);

-- The heads governance listed at the last sync, so the sync can say when governance stops
-- listing someone whose lead role the portal keeps.
CREATE TABLE governance_heads (
    committee text NOT NULL REFERENCES committees (slug),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (committee, user_id)
);
