import sqlite3

def fix_duplicates():
    try:
        conn = sqlite3.connect('vortex_server.db')
        cursor = conn.cursor()
        
        # Define canonical names (case sensitive) -> target
        # We want to map the "bad" ones (usually lower case or directory name) to the "good" ones (TMDB title)
        # We can find the "good" one by picking the one with the most episodes?
        
        print("Finding duplicates...")
        cursor.execute("SELECT series_name, COUNT(*) as c FROM media WHERE series_name IS NOT NULL GROUP BY series_name")
        rows = cursor.fetchall()
        
        # Group by case-insensitive name
        groups = {}
        for name, count in rows:
            lower = name.lower()
            if lower not in groups:
                groups[lower] = []
            groups[lower].append((name, count))
            
        for lower, variants in groups.items():
            if len(variants) > 1:
                # Found duplicates
                print(f"Detected variants for '{lower}': {variants}")
                # Sort by count descending, assume most frequent is the correct one (canonical)
                variants.sort(key=lambda x: x[1], reverse=True)
                canonical = variants[0][0]
                
                for variant, count in variants[1:]:
                    print(f"  Merging '{variant}' ({count}) -> '{canonical}'")
                    cursor.execute("UPDATE media SET series_name = ? WHERE series_name = ?", (canonical, variant))
                    
        conn.commit()
        print("Duplicates merged.")

        conn.close()
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    fix_duplicates()
