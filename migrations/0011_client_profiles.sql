-- Optional client details (decision 94): name, phone and photo, set in
-- 🔧 Settings or the phone when requesting a ride. Name and phone are
-- encrypted like the drivers'. Only the driver assigned to their trip sees
-- them. "Delete my details" removes the row. Drivers use their profile.
CREATE TABLE client_profiles (
    telegram_id   INTEGER PRIMARY KEY,
    name_enc      BLOB,
    phone_enc     BLOB,
    photo_file_id TEXT,
    updated_at    INTEGER NOT NULL
);
