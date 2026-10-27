#!/usr/bin/env python3
"""Collect real, verbatim reviews and discussions about Telegram Desktop from public APIs.
Fetches from Apple App Store RSS and Hacker News Algolia API.
Strict adherence to replica-entrepreneur: no invented text, all quotes verbatim with URLs.
"""

import csv
import html
import json
import re
import urllib.parse
import urllib.request
from datetime import datetime

OUTPUT_CSV = "replica/reviews.csv"
USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko)"


def clean_text(text):
    if not text:
        return ""
    # Strip HTML tags
    clean = re.sub(r"<[^>]+>", " ", text)
    clean = html.unescape(clean)
    # Normalize whitespaces
    clean = re.sub(r"\s+", " ", clean).strip()
    return clean


def fetch_app_store(app_id, source_name, pages=3):
    reviews = []
    for page in range(1, pages + 1):
        url = f"https://itunes.apple.com/us/rss/customerreviews/page={page}/id={app_id}/sortBy=mostRecent/json"
        req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                entries = data.get("feed", {}).get("entry", [])
                # First entry is app metadata, subsequent are user reviews
                for e in entries[1:]:
                    review_id = e.get("id", {}).get("label", "")
                    title = clean_text(e.get("title", {}).get("label", ""))
                    content = clean_text(e.get("content", {}).get("label", ""))
                    rating = e.get("im:rating", {}).get("label", "")
                    updated = e.get("updated", {}).get("label", "")[:10]
                    
                    # Direct review permalink anchor
                    link = f"https://apps.apple.com/us/app/id{app_id}#review-{review_id}" if review_id else f"https://apps.apple.com/us/app/id{app_id}"
                    full_text = f"{title}. {content}" if title else content

                    if full_text and len(full_text) > 8:
                        reviews.append({
                            "source": source_name,
                            "url": link,
                            "date": updated or datetime.now().strftime("%Y-%m-%d"),
                            "rating": rating,
                            "text": full_text
                        })
        except Exception as exc:
            print(f"Error fetching App Store {app_id} page {page}: {exc}")
            break

    return reviews


def fetch_hn_comments(query, limit=30):
    escaped_q = urllib.parse.quote_plus(query)
    url = f"https://hn.algolia.com/api/v1/search?query={escaped_q}&tags=comment&hitsPerPage={limit}"
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    reviews = []

    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            hits = data.get("hits", [])
            for h in hits:
                comment_text = clean_text(h.get("comment_text", ""))
                object_id = h.get("objectID")
                created_at = h.get("created_at", "")[:10]
                link = f"https://news.ycombinator.com/item?id={object_id}"

                if comment_text and len(comment_text) > 25 and ("telegram" in comment_text.lower() or "desktop" in comment_text.lower()):
                    reviews.append({
                        "source": "hacker-news",
                        "url": link,
                        "date": created_at,
                        "rating": "",
                        "text": comment_text
                    })
    except Exception as exc:
        print(f"Error fetching HN query '{query}': {exc}")

    return reviews


def main():
    all_reviews = []

    print("Fetching App Store reviews for Telegram Desktop (macOS, id 747648890)...")
    desktop_reviews = fetch_app_store("747648890", "app-store-desktop", pages=3)
    print(f"  -> Got {len(desktop_reviews)} desktop reviews")
    all_reviews.extend(desktop_reviews)

    print("Fetching App Store reviews for Telegram (iOS, id 686449807)...")
    mobile_reviews = fetch_app_store("686449807", "app-store-mobile", pages=3)
    print(f"  -> Got {len(mobile_reviews)} mobile reviews")
    all_reviews.extend(mobile_reviews)

    print("Fetching Hacker News discussions...")
    hn_queries = [
        "telegram desktop bloat",
        "telegram ram usage",
        "telegram memory leak",
        "telegram stories annoying",
        "telegram crypto wallet ton",
        "telegram secret chats desktop",
        "telegram desktop client linux",
        "telegram cli terminal client",
        "telegram webview mini apps bloat"
    ]
    for q in hn_queries:
        comments = fetch_hn_comments(q, limit=25)
        print(f"  -> Query '{q}': {len(comments)} comments")
        all_reviews.extend(comments)

    # Deduplicate by URL and Text
    seen_urls = set()
    unique_reviews = []
    for r in all_reviews:
        if r["url"] not in seen_urls and r["text"]:
            seen_urls.add(r["url"])
            unique_reviews.append(r)

    print(f"\nTotal unique reviews collected: {len(unique_reviews)}")

    with open(OUTPUT_CSV, "w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=["source", "url", "date", "rating", "text"])
        writer.writeheader()
        writer.writerows(unique_reviews)

    print(f"Written to {OUTPUT_CSV}")


if __name__ == "__main__":
    main()
