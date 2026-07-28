def check_partitions():
    lut = [0, 0, 2048, 4869, 8192, 11888, 15882, 20123, 24576, 29214, 34017, 38967, 44052, 49260, 54582, 60010, 65536, 71155, 76860, 82648, 88513, 94452, 100462, 106539, 112680, 118883, 125145, 131463, 137836, 144263, 150740, 157266, 163840, 170460, 177125, 183834, 190584, 197376, 204207, 211078, 217986, 224931, 231913, 238929, 245980, 253065, 260182, 267331, 274512, 281724, 288965, 296237, 303537, 310866, 318222, 325606, 333017, 340454, 347917, 355406, 362919, 370458, 378020, 385606, 393216]
    
    # We want to maximize sum(lut[c]) over all valid combinations of c_i summing to 56.
    # We can use dynamic programming.
    # dp[i] is max sum of LUT values for partition of i.
    dp = [0] * 57
    for i in range(1, 57):
        dp[i] = max([dp[i-c] + lut[c] for c in range(1, i+1)])
        
    print(f"Max LUT sum for 56: {dp[56]}")
    print(f"LUT[56]: {lut[56]}")
    
    # What if we MINIMIZE to see if it can go negative? 
    # Not relevant, saturating_sub prevents negative.
    # But wait, could sum > LUT[56]?
    if dp[56] > lut[56]:
        print("ALERT: Sum exceeds LUT[56]")
    else:
        print("OK: Sum never exceeds LUT[56]")

check_partitions()
