//! Database operations for Bookwyrm-style book tracking.
//!
//! Pure CRUD operations against the `books`, `reading_shelves`, and
//! `book_reviews` tables defined in [`crate::db`]. Each function dispatches
//! on [`crate::db::Pool`] to emit the correct SQL dialect.

use crate::db::Pool;

// ---------------------------------------------------------------------------
// Book rows
// ---------------------------------------------------------------------------

/// A row from the `books` table.
#[derive(Debug, Clone)]
pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub isbn: Option<String>,
    pub isbn13: Option<String>,
    pub openlibrary_id: Option<String>,
    pub cover_url: Option<String>,
    pub description: String,
    pub pages: Option<i32>,
    pub published_year: Option<i32>,
    pub language: Option<String>,
    pub created_at: i64,
}

type BookTuple = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<i32>,
    Option<i32>,
    Option<String>,
    i64,
);

fn tuple_to_book(r: BookTuple) -> BookRow {
    BookRow {
        id: r.0,
        title: r.1,
        author: r.2,
        isbn: r.3,
        isbn13: r.4,
        openlibrary_id: r.5,
        cover_url: r.6,
        description: r.7,
        pages: r.8,
        published_year: r.9,
        language: r.10,
        created_at: r.11,
    }
}

const BOOK_COLS: &str = "id, title, author, isbn, isbn13, openlibrary_id, \
    cover_url, description, pages, published_year, language, created_at";

/// Insert a new book.
pub async fn create_book(pool: &Pool, book: &BookRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO books (id, title, author, isbn, isbn13, openlibrary_id, \
                 cover_url, description, pages, published_year, language, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(book.id)
            .bind(&book.title)
            .bind(&book.author)
            .bind(&book.isbn)
            .bind(&book.isbn13)
            .bind(&book.openlibrary_id)
            .bind(&book.cover_url)
            .bind(&book.description)
            .bind(book.pages)
            .bind(book.published_year)
            .bind(&book.language)
            .bind(book.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO books (id, title, author, isbn, isbn13, openlibrary_id, \
                 cover_url, description, pages, published_year, language, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .bind(book.id)
            .bind(&book.title)
            .bind(&book.author)
            .bind(&book.isbn)
            .bind(&book.isbn13)
            .bind(&book.openlibrary_id)
            .bind(&book.cover_url)
            .bind(&book.description)
            .bind(book.pages)
            .bind(book.published_year)
            .bind(&book.language)
            .bind(book.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a book by ID.
pub async fn get_book(pool: &Pool, id: i64) -> Result<Option<BookRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<BookTuple> =
                sqlx::query_as(&format!("SELECT {BOOK_COLS} FROM books WHERE id = $1"))
                    .bind(id)
                    .fetch_optional(pg)
                    .await?;
            Ok(row.map(tuple_to_book))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<BookTuple> =
                sqlx::query_as(&format!("SELECT {BOOK_COLS} FROM books WHERE id = ?1"))
                    .bind(id)
                    .fetch_optional(sq)
                    .await?;
            Ok(row.map(tuple_to_book))
        }
    }
}

/// Search books by title or author (LIKE match).
pub async fn search_books(
    pool: &Pool,
    query: &str,
    limit: i64,
) -> Result<Vec<BookRow>, sqlx::Error> {
    let pattern = format!("%{query}%");
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<BookTuple> = sqlx::query_as(&format!(
                "SELECT {BOOK_COLS} FROM books \
                 WHERE title LIKE $1 OR author LIKE $1 \
                 ORDER BY title LIMIT $2"
            ))
            .bind(&pattern)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_book).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<BookTuple> = sqlx::query_as(&format!(
                "SELECT {BOOK_COLS} FROM books \
                 WHERE title LIKE ?1 OR author LIKE ?1 \
                 ORDER BY title LIMIT ?2"
            ))
            .bind(&pattern)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_book).collect())
        }
    }
}

/// Look up a book by ISBN (checks both isbn and isbn13 columns).
pub async fn get_book_by_isbn(pool: &Pool, isbn: &str) -> Result<Option<BookRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<BookTuple> = sqlx::query_as(&format!(
                "SELECT {BOOK_COLS} FROM books WHERE isbn = $1 OR isbn13 = $1 LIMIT 1"
            ))
            .bind(isbn)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(tuple_to_book))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<BookTuple> = sqlx::query_as(&format!(
                "SELECT {BOOK_COLS} FROM books WHERE isbn = ?1 OR isbn13 = ?1 LIMIT 1"
            ))
            .bind(isbn)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(tuple_to_book))
        }
    }
}

// ---------------------------------------------------------------------------
// Reading shelves
// ---------------------------------------------------------------------------

/// Set or update the reading status for a user+book pair (UPSERT).
pub async fn set_reading_status(
    pool: &Pool,
    user_id: i64,
    book_id: i64,
    status: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO reading_shelves (user_id, book_id, status, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $4) \
                 ON CONFLICT (user_id, book_id) DO UPDATE SET status = $3, updated_at = $4",
            )
            .bind(user_id)
            .bind(book_id)
            .bind(status)
            .bind(now)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO reading_shelves (user_id, book_id, status, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4) \
                 ON CONFLICT (user_id, book_id) DO UPDATE SET status = ?3, updated_at = ?4",
            )
            .bind(user_id)
            .bind(book_id)
            .bind(status)
            .bind(now)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get the reading status for a user+book pair.
/// Returns (status, started_at, finished_at, rating, updated_at).
pub async fn get_reading_status(
    pool: &Pool,
    user_id: i64,
    book_id: i64,
) -> Result<Option<(String, Option<i64>, Option<i64>, Option<i32>, i64)>, sqlx::Error> {
    let row: Option<(String, Option<i64>, Option<i64>, Option<i32>, i64)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT status, started_at, finished_at, rating, \
                 COALESCE(updated_at, created_at) \
                 FROM reading_shelves WHERE user_id = $1 AND book_id = $2",
            )
            .bind(user_id)
            .bind(book_id)
            .fetch_optional(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT status, started_at, finished_at, rating, \
                 COALESCE(updated_at, created_at) \
                 FROM reading_shelves WHERE user_id = ?1 AND book_id = ?2",
            )
            .bind(user_id)
            .bind(book_id)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(row)
}

/// Rate a book (updates existing reading_shelves row).
pub async fn rate_book(
    pool: &Pool,
    user_id: i64,
    book_id: i64,
    rating: i32,
    now: i64,
) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "UPDATE reading_shelves SET rating = $1, updated_at = $2 \
                 WHERE user_id = $3 AND book_id = $4",
            )
            .bind(rating)
            .bind(now)
            .bind(user_id)
            .bind(book_id)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE reading_shelves SET rating = ?1, updated_at = ?2 \
                 WHERE user_id = ?3 AND book_id = ?4",
            )
            .bind(rating)
            .bind(now)
            .bind(user_id)
            .bind(book_id)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// List books on a user's shelf by status.
/// Returns (book_id, title, author, cover_url, rating).
pub async fn user_shelf(
    pool: &Pool,
    user_id: i64,
    status: &str,
    limit: i64,
) -> Result<Vec<(i64, String, String, Option<String>, Option<i32>)>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<(i64, String, String, Option<String>, Option<i32>)> = sqlx::query_as(
                "SELECT b.id, b.title, b.author, b.cover_url, s.rating \
                 FROM reading_shelves s \
                 JOIN books b ON b.id = s.book_id \
                 WHERE s.user_id = $1 AND s.status = $2 \
                 ORDER BY COALESCE(s.updated_at, s.created_at) DESC \
                 LIMIT $3",
            )
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows)
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<(i64, String, String, Option<String>, Option<i32>)> = sqlx::query_as(
                "SELECT b.id, b.title, b.author, b.cover_url, s.rating \
                 FROM reading_shelves s \
                 JOIN books b ON b.id = s.book_id \
                 WHERE s.user_id = ?1 AND s.status = ?2 \
                 ORDER BY COALESCE(s.updated_at, s.created_at) DESC \
                 LIMIT ?3",
            )
            .bind(user_id)
            .bind(status)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows)
        }
    }
}

/// Get reading stats for a user: (to_read, reading, read) counts.
pub async fn reading_stats(
    pool: &Pool,
    user_id: i64,
) -> Result<(i64, i64, i64), sqlx::Error> {
    let rows: Vec<(String, i64)> = match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT status, COUNT(*) FROM reading_shelves \
                 WHERE user_id = $1 GROUP BY status",
            )
            .bind(user_id)
            .fetch_all(pg)
            .await?
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT status, COUNT(*) FROM reading_shelves \
                 WHERE user_id = ?1 GROUP BY status",
            )
            .bind(user_id)
            .fetch_all(sq)
            .await?
        }
    };

    let mut to_read = 0i64;
    let mut reading = 0i64;
    let mut read = 0i64;
    for (status, count) in rows {
        match status.as_str() {
            "to-read" => to_read = count,
            "reading" => reading = count,
            "read" => read = count,
            _ => {}
        }
    }
    Ok((to_read, reading, read))
}

// ---------------------------------------------------------------------------
// Reviews
// ---------------------------------------------------------------------------

/// A row from the `book_reviews` table.
#[derive(Debug, Clone)]
pub struct ReviewRow {
    pub id: i64,
    pub user_id: i64,
    pub persona_id: i64,
    pub book_id: i64,
    pub content: String,
    pub content_html: String,
    pub rating: Option<i32>,
    pub spoiler: bool,
    pub ap_id: String,
    pub created_at: i64,
}

type ReviewTuple = (
    i64,
    i64,
    i64,
    i64,
    String,
    String,
    Option<i32>,
    i32,
    String,
    i64,
);

fn tuple_to_review(r: ReviewTuple) -> ReviewRow {
    ReviewRow {
        id: r.0,
        user_id: r.1,
        persona_id: r.2,
        book_id: r.3,
        content: r.4,
        content_html: r.5,
        rating: r.6,
        spoiler: r.7 != 0,
        ap_id: r.8,
        created_at: r.9,
    }
}

const REVIEW_COLS: &str = "id, user_id, persona_id, book_id, content, content_html, \
    rating, spoiler, ap_id, created_at";

/// Insert a new book review.
pub async fn create_review(pool: &Pool, review: &ReviewRow) -> Result<(), sqlx::Error> {
    let spoiler_int: i32 = if review.spoiler { 1 } else { 0 };
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO book_reviews (id, user_id, persona_id, book_id, content, \
                 content_html, rating, spoiler, ap_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(review.id)
            .bind(review.user_id)
            .bind(review.persona_id)
            .bind(review.book_id)
            .bind(&review.content)
            .bind(&review.content_html)
            .bind(review.rating)
            .bind(spoiler_int)
            .bind(&review.ap_id)
            .bind(review.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO book_reviews (id, user_id, persona_id, book_id, content, \
                 content_html, rating, spoiler, ap_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(review.id)
            .bind(review.user_id)
            .bind(review.persona_id)
            .bind(review.book_id)
            .bind(&review.content)
            .bind(&review.content_html)
            .bind(review.rating)
            .bind(spoiler_int)
            .bind(&review.ap_id)
            .bind(review.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Get a review by ID.
pub async fn get_review(pool: &Pool, id: i64) -> Result<Option<ReviewRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let row: Option<ReviewTuple> =
                sqlx::query_as(&format!("SELECT {REVIEW_COLS} FROM book_reviews WHERE id = $1"))
                    .bind(id)
                    .fetch_optional(pg)
                    .await?;
            Ok(row.map(tuple_to_review))
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let row: Option<ReviewTuple> =
                sqlx::query_as(&format!("SELECT {REVIEW_COLS} FROM book_reviews WHERE id = ?1"))
                    .bind(id)
                    .fetch_optional(sq)
                    .await?;
            Ok(row.map(tuple_to_review))
        }
    }
}

/// Get reviews for a book, ordered by newest first.
pub async fn reviews_for_book(
    pool: &Pool,
    book_id: i64,
    limit: i64,
) -> Result<Vec<ReviewRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM book_reviews \
                 WHERE book_id = $1 ORDER BY created_at DESC LIMIT $2"
            ))
            .bind(book_id)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM book_reviews \
                 WHERE book_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ))
            .bind(book_id)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
    }
}

/// Get reviews by a user, ordered by newest first.
pub async fn reviews_by_user(
    pool: &Pool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<ReviewRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "postgres")]
        Pool::Postgres(pg) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM book_reviews \
                 WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"
            ))
            .bind(user_id)
            .bind(limit)
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
        #[cfg(feature = "sqlite")]
        Pool::Sqlite(sq) => {
            let rows: Vec<ReviewTuple> = sqlx::query_as(&format!(
                "SELECT {REVIEW_COLS} FROM book_reviews \
                 WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2"
            ))
            .bind(user_id)
            .bind(limit)
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(tuple_to_review).collect())
        }
    }
}

#[cfg(test)]
#[cfg(feature = "sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> (Pool, sqlx::SqlitePool) {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES (1, 'test@example.com', 'Test', 'user', 1000); \
             INSERT INTO personas (id, user_id, username, private_key_pem, public_key_pem, created_at) \
             VALUES (101, 1, 'alice', 'pk', 'pub', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        (pool, sq)
    }

    fn sample_book(id: i64) -> BookRow {
        BookRow {
            id,
            title: "The Left Hand of Darkness".into(),
            author: "Ursula K. Le Guin".into(),
            isbn: Some("0441478123".into()),
            isbn13: Some("9780441478125".into()),
            openlibrary_id: Some("OL1234W".into()),
            cover_url: None,
            description: "A novel of gender and politics on a distant planet.".into(),
            pages: Some(304),
            published_year: Some(1969),
            language: Some("en".into()),
            created_at: 2000,
        }
    }

    fn sample_review(id: i64, book_id: i64) -> ReviewRow {
        ReviewRow {
            id,
            user_id: 1,
            persona_id: 101,
            book_id,
            content: "A masterpiece of science fiction.".into(),
            content_html: "<p>A masterpiece of science fiction.</p>".into(),
            rating: Some(5),
            spoiler: false,
            ap_id: format!("https://example.com/reviews/{id}"),
            created_at: 3000,
        }
    }

    // -- Book tests --

    #[tokio::test]
    async fn create_and_get_book() {
        let (pool, _) = setup().await;
        let book = sample_book(1);
        create_book(&pool, &book).await.unwrap();

        let fetched = get_book(&pool, 1).await.unwrap().expect("book should exist");
        assert_eq!(fetched.title, "The Left Hand of Darkness");
        assert_eq!(fetched.author, "Ursula K. Le Guin");
        assert_eq!(fetched.isbn, Some("0441478123".into()));
        assert_eq!(fetched.pages, Some(304));
    }

    #[tokio::test]
    async fn get_nonexistent_book() {
        let (pool, _) = setup().await;
        assert!(get_book(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn search_books_by_title() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let mut book2 = sample_book(2);
        book2.title = "A Wizard of Earthsea".into();
        book2.isbn = Some("0553383043".into());
        book2.isbn13 = None;
        create_book(&pool, &book2).await.unwrap();

        let results = search_books(&pool, "Darkness", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Left Hand of Darkness");

        let results = search_books(&pool, "Le Guin", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn get_book_by_isbn_works() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let found = get_book_by_isbn(&pool, "0441478123").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 1);

        let found13 = get_book_by_isbn(&pool, "9780441478125").await.unwrap();
        assert!(found13.is_some());

        let missing = get_book_by_isbn(&pool, "0000000000").await.unwrap();
        assert!(missing.is_none());
    }

    // -- Shelf tests --

    #[tokio::test]
    async fn set_and_get_reading_status() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        set_reading_status(&pool, 1, 1, "to-read", 2000).await.unwrap();
        let status = get_reading_status(&pool, 1, 1).await.unwrap().unwrap();
        assert_eq!(status.0, "to-read");
        assert!(status.3.is_none()); // no rating yet
    }

    #[tokio::test]
    async fn upsert_reading_status() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        set_reading_status(&pool, 1, 1, "to-read", 2000).await.unwrap();
        set_reading_status(&pool, 1, 1, "reading", 3000).await.unwrap();

        let status = get_reading_status(&pool, 1, 1).await.unwrap().unwrap();
        assert_eq!(status.0, "reading");
        assert_eq!(status.4, 3000); // updated_at
    }

    #[tokio::test]
    async fn rate_book_updates_rating() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();
        set_reading_status(&pool, 1, 1, "read", 2000).await.unwrap();

        rate_book(&pool, 1, 1, 4, 3000).await.unwrap();
        let status = get_reading_status(&pool, 1, 1).await.unwrap().unwrap();
        assert_eq!(status.3, Some(4));
    }

    #[tokio::test]
    async fn user_shelf_lists_books() {
        let (pool, _) = setup().await;
        let book1 = sample_book(1);
        create_book(&pool, &book1).await.unwrap();

        let mut book2 = sample_book(2);
        book2.title = "A Wizard of Earthsea".into();
        book2.isbn = None;
        book2.isbn13 = None;
        create_book(&pool, &book2).await.unwrap();

        set_reading_status(&pool, 1, 1, "to-read", 2000).await.unwrap();
        set_reading_status(&pool, 1, 2, "to-read", 3000).await.unwrap();

        let shelf = user_shelf(&pool, 1, "to-read", 10).await.unwrap();
        assert_eq!(shelf.len(), 2);
        // Most recently updated first
        assert_eq!(shelf[0].0, 2); // book_id=2 updated at 3000
        assert_eq!(shelf[1].0, 1); // book_id=1 updated at 2000
    }

    #[tokio::test]
    async fn reading_stats_counts() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let mut book2 = sample_book(2);
        book2.title = "Book 2".into();
        book2.isbn = None;
        book2.isbn13 = None;
        create_book(&pool, &book2).await.unwrap();

        let mut book3 = sample_book(3);
        book3.title = "Book 3".into();
        book3.isbn = Some("1111111111".into());
        book3.isbn13 = None;
        create_book(&pool, &book3).await.unwrap();

        set_reading_status(&pool, 1, 1, "to-read", 2000).await.unwrap();
        set_reading_status(&pool, 1, 2, "reading", 2000).await.unwrap();
        set_reading_status(&pool, 1, 3, "read", 2000).await.unwrap();

        let (to_read, reading, read) = reading_stats(&pool, 1).await.unwrap();
        assert_eq!(to_read, 1);
        assert_eq!(reading, 1);
        assert_eq!(read, 1);
    }

    #[tokio::test]
    async fn reading_stats_empty() {
        let (pool, _) = setup().await;
        let (to_read, reading, read) = reading_stats(&pool, 1).await.unwrap();
        assert_eq!(to_read, 0);
        assert_eq!(reading, 0);
        assert_eq!(read, 0);
    }

    #[tokio::test]
    async fn get_reading_status_missing() {
        let (pool, _) = setup().await;
        assert!(get_reading_status(&pool, 1, 999).await.unwrap().is_none());
    }

    // -- Review tests --

    #[tokio::test]
    async fn create_and_get_review() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();
        set_reading_status(&pool, 1, 1, "read", 2000).await.unwrap();

        let review = sample_review(1, 1);
        create_review(&pool, &review).await.unwrap();

        let fetched = get_review(&pool, 1).await.unwrap().expect("review should exist");
        assert_eq!(fetched.content, "A masterpiece of science fiction.");
        assert_eq!(fetched.rating, Some(5));
        assert!(!fetched.spoiler);
    }

    #[tokio::test]
    async fn get_nonexistent_review() {
        let (pool, _) = setup().await;
        assert!(get_review(&pool, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reviews_for_book_ordered() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let mut r1 = sample_review(1, 1);
        r1.created_at = 3000;
        create_review(&pool, &r1).await.unwrap();

        let mut r2 = sample_review(2, 1);
        r2.ap_id = "https://example.com/reviews/2".into();
        r2.created_at = 4000;
        create_review(&pool, &r2).await.unwrap();

        let reviews = reviews_for_book(&pool, 1, 10).await.unwrap();
        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].id, 2); // newer first
        assert_eq!(reviews[1].id, 1);
    }

    #[tokio::test]
    async fn reviews_by_user_ordered() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let mut book2 = sample_book(2);
        book2.title = "Book 2".into();
        book2.isbn = None;
        book2.isbn13 = None;
        create_book(&pool, &book2).await.unwrap();

        let mut r1 = sample_review(1, 1);
        r1.created_at = 3000;
        create_review(&pool, &r1).await.unwrap();

        let mut r2 = sample_review(2, 2);
        r2.ap_id = "https://example.com/reviews/2".into();
        r2.created_at = 4000;
        create_review(&pool, &r2).await.unwrap();

        let reviews = reviews_by_user(&pool, 1, 10).await.unwrap();
        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].book_id, 2); // newer first
        assert_eq!(reviews[1].book_id, 1);
    }

    #[tokio::test]
    async fn review_with_spoiler() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();

        let mut review = sample_review(1, 1);
        review.spoiler = true;
        create_review(&pool, &review).await.unwrap();

        let fetched = get_review(&pool, 1).await.unwrap().unwrap();
        assert!(fetched.spoiler);
    }

    #[tokio::test]
    async fn reviews_for_book_empty() {
        let (pool, _) = setup().await;
        create_book(&pool, &sample_book(1)).await.unwrap();
        let reviews = reviews_for_book(&pool, 1, 10).await.unwrap();
        assert!(reviews.is_empty());
    }

    #[tokio::test]
    async fn reviews_by_user_empty() {
        let (pool, _) = setup().await;
        let reviews = reviews_by_user(&pool, 1, 10).await.unwrap();
        assert!(reviews.is_empty());
    }
}
