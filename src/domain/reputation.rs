//! Aggregate reputation and blind ratings. Only totals per person are kept:
//! never which trip, with whom or where.

use super::request::UnixTime;

/// How long both sides have to rate a finished trip.
pub const RATING_WINDOW_SECS: i64 = 24 * 60 * 60;
/// The same client-driver pair counts once per this period.
pub const PAIR_WINDOW_SECS: i64 = 7 * 24 * 60 * 60;
/// Below this many trips, stars are shown as "New".
pub const NEW_UNTIL_TRIPS: i64 = 5;
/// An average below this, with at least `LOW_RATING_MIN_COUNT` ratings, alerts the admins.
pub const LOW_RATING_AVERAGE: f64 = 3.5;
pub const LOW_RATING_MIN_COUNT: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Client,
    Driver,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Client => "client",
            Role::Driver => "driver",
        }
    }
}

/// Fixed, optional tags; no free comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Punctual,
    Kind,
    Overcharged,
    NoShow,
}

impl Tag {
    pub const ALL: [Tag; 4] = [Tag::Punctual, Tag::Kind, Tag::Overcharged, Tag::NoShow];

    pub fn bit(self) -> i64 {
        match self {
            Tag::Punctual => 1,
            Tag::Kind => 2,
            Tag::Overcharged => 4,
            Tag::NoShow => 8,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Tag::Punctual => "p",
            Tag::Kind => "k",
            Tag::Overcharged => "o",
            Tag::NoShow => "n",
        }
    }

    pub fn parse(code: &str) -> Option<Tag> {
        Tag::ALL.into_iter().find(|t| t.code() == code)
    }

    /// Tags a rater can give: a client does not "overcharge", a driver is not
    /// "punctual" in the same sense, but both can be kind or not show up.
    pub fn for_rating(rated: Role) -> &'static [Tag] {
        match rated {
            Role::Driver => &[Tag::Punctual, Tag::Kind, Tag::Overcharged, Tag::NoShow],
            Role::Client => &[Tag::Punctual, Tag::Kind, Tag::NoShow],
        }
    }
}

/// One side's rating: 1 to 5 stars plus a set of tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rating {
    pub stars: i64,
    pub tags: i64,
}

impl Rating {
    pub fn has(&self, tag: Tag) -> bool {
        self.tags & tag.bit() != 0
    }
}

pub fn valid_stars(stars: i64) -> bool {
    (1..=5).contains(&stars)
}

/// A person's aggregates in one role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reputation {
    pub first_use: UnixTime,
    pub trips: i64,
    pub stars_sum: i64,
    pub stars_count: i64,
}

/// What others see.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    /// `None` while the person is "New".
    pub stars: Option<f64>,
    pub trips: i64,
    pub days: i64,
}

impl Reputation {
    pub fn average(&self) -> Option<f64> {
        (self.stars_count > 0).then(|| self.stars_sum as f64 / self.stars_count as f64)
    }

    pub fn shown(&self, now: UnixTime) -> Shown {
        let stars = if self.trips < NEW_UNTIL_TRIPS { None } else { self.average() };
        Shown { stars, trips: self.trips, days: ((now - self.first_use).max(0)) / 86_400 }
    }

    fn is_low(&self) -> bool {
        self.stars_count >= LOW_RATING_MIN_COUNT && self.average().is_some_and(|a| a < LOW_RATING_AVERAGE)
    }

    /// Whether adding `rating` makes the average fall into the "low" zone,
    /// so the admins are alerted once, when it happens.
    pub fn becomes_low_with(&self, rating: Rating) -> bool {
        let after = Reputation {
            stars_sum: self.stars_sum + rating.stars,
            stars_count: self.stars_count + 1,
            ..*self
        };
        !self.is_low() && after.is_low()
    }
}

/// Cancellations that count against a client within this window block them.
pub const CANCELLATION_WINDOW_SECS: i64 = 7 * 24 * 60 * 60;
pub const CANCELLATIONS_TO_BLOCK: i64 = 3;
pub const CANCELLATION_BLOCK_SECS: i64 = 24 * 60 * 60;
/// Cancellation records are deleted after this.
pub const CANCELLATION_TTL_SECS: i64 = 30 * 24 * 60 * 60;

/// With `recent` counted cancellations in the last 7 days (this one
/// included), whether the client gets the 24-hour block.
pub fn cancellation_blocks(recent: i64) -> bool {
    recent >= CANCELLATIONS_TO_BLOCK
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    #[test]
    fn new_people_show_no_stars() {
        let r = Reputation { first_use: 0, trips: 4, stars_sum: 20, stars_count: 4 };
        assert_eq!(r.shown(3 * DAY + 5), Shown { stars: None, trips: 4, days: 3 });
        let r = Reputation { trips: 5, ..r };
        assert_eq!(r.shown(0).stars, Some(5.0));
    }

    #[test]
    fn low_rating_alerts_once_when_crossing() {
        let ok = Reputation { first_use: 0, trips: 10, stars_sum: 36, stars_count: 10 }; // 3.6
        assert!(ok.becomes_low_with(Rating { stars: 1, tags: 0 })); // 37/11 ≈ 3.36
        let low = Reputation { stars_sum: 33, ..ok }; // 3.3, already low
        assert!(!low.becomes_low_with(Rating { stars: 1, tags: 0 }));
        let few = Reputation { stars_sum: 9, stars_count: 3, ..ok };
        assert!(!few.becomes_low_with(Rating { stars: 1, tags: 0 }));
    }

    #[test]
    fn tags_round_trip_and_fit_the_role() {
        for t in Tag::ALL {
            assert_eq!(Tag::parse(t.code()), Some(t));
        }
        assert!(!Tag::for_rating(Role::Client).contains(&Tag::Overcharged));
        let r = Rating { stars: 4, tags: Tag::Kind.bit() | Tag::NoShow.bit() };
        assert!(r.has(Tag::Kind) && r.has(Tag::NoShow) && !r.has(Tag::Punctual));
        assert!(valid_stars(1) && valid_stars(5) && !valid_stars(0) && !valid_stars(6));
    }

    #[test]
    fn three_cancellations_in_a_week_block() {
        assert!(!cancellation_blocks(2));
        assert!(cancellation_blocks(3));
    }
}
